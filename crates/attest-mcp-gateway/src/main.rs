//! MCP Gateway binary — starts the axum HTTP server on `MCP_GATEWAY_PORT` (default 4242).

use attest_mcp_gateway::{build_router, registry::default_registry};
use axum::middleware::from_fn;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service_name =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "attest-mcp-gateway".into());
    let _otel = attest_telemetry::init_subscriber_with_otel(&service_name)?;

    let port: u16 = std::env::var("MCP_GATEWAY_PORT")
        .unwrap_or_else(|_| "4242".into())
        .parse()?;

    let policy = match std::env::var("ATTEST_POLICY_FILE") {
        Ok(path) => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("invalid policy file: {path}: {e}"))?;
            attest_policy_engine::file::PolicyFile::parse(&text).map_err(|e| anyhow::anyhow!(e))?
        }
        Err(_) => attest_policy_engine::file::PolicyFile::builtin(),
    };
    policy.install();
    tracing::info!(hash = %policy.hash(), "policy file loaded");

    let registry = default_registry();
    let secret = std::env::var(attest_service_auth::SERVICE_AUTH_ENV).unwrap_or_default();
    let router = attest_service_auth::protect(
        build_router(registry),
        secret,
        attest_service_auth::AUDIENCE_MCP_GATEWAY,
    )
    .layer(from_fn(attest_telemetry::axum_trace_propagation));

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "MCP gateway listening");

    axum::serve(listener, router).await?;
    Ok(())
}
