//! OpenTelemetry OTLP tracing (Grafana Tempo–compatible) and W3C trace propagation.
//!
//! When `OTEL_EXPORTER_OTLP_ENDPOINT` is unset or empty, telemetry init is a no-op
//! and only the `fmt` layer should be registered by the binary.

use anyhow::Context;
use axum::{extract::Request, middleware::Next, response::Response};
use opentelemetry::propagation::Injector;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    resource::Resource,
    trace::{Sampler, TracerProvider},
};
use std::sync::OnceLock;
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

static PROPAGATOR_READY: OnceLock<()> = OnceLock::new();

fn ensure_propagator() {
    PROPAGATOR_READY.get_or_init(|| {
        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    });
}

/// Build an OTLP `TracerProvider` and register it globally. Caller must hold the
/// returned guard for the process lifetime so batches flush on drop.
pub fn init_otlp_tracer_provider(service_name: &str) -> anyhow::Result<Option<TracerProvider>> {
    let endpoint = match std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        Ok(e) if !e.trim().is_empty() => e.trim().to_string(),
        _ => return Ok(None),
    };

    ensure_propagator();

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&endpoint)
        .build()
        .context("failed to build OTLP span exporter — check OTEL_EXPORTER_OTLP_ENDPOINT")?;

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_sampler(Sampler::ParentBased(Box::new(Sampler::AlwaysOn)))
        .with_resource(Resource::new([KeyValue::new(
            "service.name",
            service_name.to_string(),
        )]))
        .build();

    opentelemetry::global::set_tracer_provider(provider.clone());
    Ok(Some(provider))
}

/// Initialise `tracing` with env filter, optional OTLP layer, and a stdout `fmt` layer.
///
/// `service_name` becomes the OpenTelemetry `service.name` resource attribute.
pub fn init_subscriber_with_otel(service_name: &str) -> anyhow::Result<Option<TracerProvider>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer();

    match init_otlp_tracer_provider(service_name)? {
        Some(provider) => {
            let tracer = provider.tracer(service_name.to_string());
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            Registry::default()
                .with(filter)
                .with(fmt_layer)
                .with(otel_layer)
                .init();
            Ok(Some(provider))
        }
        None => {
            Registry::default().with(filter).with(fmt_layer).init();
            Ok(None)
        }
    }
}

struct HeaderExtractor<'a>(&'a http::HeaderMap);

impl opentelemetry::propagation::Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

struct HeaderInjector<'a>(&'a mut http::HeaderMap);

impl Injector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(name) = http::HeaderName::try_from(key) {
            if let Ok(val) = http::HeaderValue::try_from(value) {
                self.0.insert(name, val);
            }
        }
    }
}

/// Axum middleware: resume parent trace from `traceparent` / `tracestate`.
pub async fn axum_trace_propagation(request: Request, next: Next) -> Response {
    ensure_propagator();
    let parent = opentelemetry::global::get_text_map_propagator(|p| {
        p.extract(&HeaderExtractor(request.headers()))
    });

    let method = request.method().clone();
    let path = request.uri().path().to_string();

    let span = tracing::info_span!(
        "request",
        http.method = %method,
        http.route = %path,
    );
    tracing_opentelemetry::OpenTelemetrySpanExt::set_parent(&span, parent);

    next.run(request).instrument(span).await
}

/// Inject W3C trace context from the current span into an outgoing request.
pub fn inject_trace_headers(headers: &mut http::HeaderMap) {
    ensure_propagator();
    let ctx = tracing_opentelemetry::OpenTelemetrySpanExt::context(&tracing::Span::current());
    let mut inj = HeaderInjector(headers);
    opentelemetry::global::get_text_map_propagator(|p| p.inject_context(&ctx, &mut inj));
}

#[cfg(test)]
mod tests {
    #[test]
    fn noop_without_endpoint() {
        std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
        let r = super::init_otlp_tracer_provider("test").unwrap();
        assert!(r.is_none());
    }
}
