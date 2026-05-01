//! Direct-to-Kafka load generator with optional sampled ML triage.
//!
//! N_PRODUCERS parallel tokio tasks write to the `cloudtrail` Kafka topic.
//! When `sampled_triage_pct > 0`, an additional sampler task concurrently
//! sends that percentage of events to the orchestrator `POST /triage` and
//! tracks latency in a separate HdrHistogram exposed on `/status`.

use anyhow::Result;
use hdrhistogram::Histogram;
use rdkafka::{
    config::ClientConfig,
    producer::{FutureProducer, FutureRecord},
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

use crate::scenarios;

const N_PRODUCERS: usize = 8;
const TOPIC: &str = "cloudtrail";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    #[default]
    Mixed,
    Attack,
    Benign,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConfig {
    /// Target events per second (global, across all producers).
    pub rate: u64,
    /// How long to run.  0 = run until `POST /stop`.
    pub duration_secs: u64,
    pub scenario: Scenario,
    /// Number of simulated tenants (each gets its own user identities).
    pub tenants: usize,
    /// Pre-seed baselines before starting — ensures geo-anomaly detections fire.
    pub seed_baselines: bool,
    /// Percentage of events also sent to the orchestrator for ML triage (0–100).
    /// 0 = streaming-rules only (no triage latency data, maximum throughput).
    /// 5 = 5% sampled → gives real triage p95 without overwhelming the orchestrator.
    #[serde(default)]
    pub sampled_triage_pct: u8,
}

impl Default for RunConfig {
    fn default() -> Self {
        RunConfig {
            rate: 10_000,
            duration_secs: 30,
            scenario: Scenario::Mixed,
            tenants: 3,
            seed_baselines: true,
            sampled_triage_pct: 0,
        }
    }
}

/// Live statistics snapshot (serialised for `/status`).
#[derive(Debug, Clone, Serialize)]
pub struct RunStatus {
    pub running: bool,
    pub sent: u64,
    pub errors: u64,
    pub elapsed_secs: f64,
    pub rate_actual: f64,
    /// Kafka publish latency percentiles (µs → ms).
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    /// Orchestrator triage latency percentiles — only non-zero when
    /// `sampled_triage_pct > 0`.
    pub triage_p50_ms: f64,
    pub triage_p95_ms: f64,
    pub triage_p99_ms: f64,
    pub triage_samples: u64,
}

/// Handle for an active (or finished) run.
pub struct RunHandle {
    pub sent: Arc<AtomicU64>,
    pub errors: Arc<AtomicU64>,
    pub running: Arc<AtomicBool>,
    pub started_at: Instant,
    /// Kafka publish latency histogram (µs).
    pub hist: Arc<Mutex<Histogram<u64>>>,
    /// Triage latency histogram (ms × 1000 for µs precision).
    pub triage_hist: Arc<Mutex<Histogram<u64>>>,
    pub triage_samples: Arc<AtomicU64>,
}

impl RunHandle {
    pub fn status(&self) -> RunStatus {
        let elapsed = self.started_at.elapsed().as_secs_f64();
        let sent = self.sent.load(Ordering::Relaxed);
        let hist = self.hist.lock().unwrap();
        let th = self.triage_hist.lock().unwrap();
        let ts = self.triage_samples.load(Ordering::Relaxed);
        RunStatus {
            running: self.running.load(Ordering::Relaxed),
            sent,
            errors: self.errors.load(Ordering::Relaxed),
            elapsed_secs: elapsed,
            rate_actual: if elapsed > 0.0 { sent as f64 / elapsed } else { 0.0 },
            p50_ms: hist.value_at_quantile(0.50) as f64 / 1000.0,
            p95_ms: hist.value_at_quantile(0.95) as f64 / 1000.0,
            p99_ms: hist.value_at_quantile(0.99) as f64 / 1000.0,
            triage_p50_ms: if ts > 0 { th.value_at_quantile(0.50) as f64 / 1000.0 } else { 0.0 },
            triage_p95_ms: if ts > 0 { th.value_at_quantile(0.95) as f64 / 1000.0 } else { 0.0 },
            triage_p99_ms: if ts > 0 { th.value_at_quantile(0.99) as f64 / 1000.0 } else { 0.0 },
            triage_samples: ts,
        }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Start the load generator and return a handle for monitoring / cancellation.
pub async fn start_run(
    brokers: String,
    cfg: RunConfig,
    orchestrator_url: Option<String>,
) -> Result<RunHandle> {
    let sent        = Arc::new(AtomicU64::new(0));
    let errors      = Arc::new(AtomicU64::new(0));
    let running     = Arc::new(AtomicBool::new(true));
    let hist        = Arc::new(Mutex::new(Histogram::new(3).expect("histogram")));
    let triage_hist = Arc::new(Mutex::new(Histogram::new(3).expect("triage histogram")));
    let triage_samples = Arc::new(AtomicU64::new(0));

    // Build tenant/user pairs
    let tenants: Vec<(String, String)> = (0..cfg.tenants)
        .map(|i| (
            format!("tenant-{:03}", i),
            format!("user-{:03}@acme.example.com", i),
        ))
        .collect();

    // Pre-seed baselines if requested
    if cfg.seed_baselines {
        seed_baselines(&brokers, &tenants).await;
    }

    let per_producer_rate = (cfg.rate as usize).max(N_PRODUCERS) / N_PRODUCERS;
    let interval_us = 1_000_000u64 / per_producer_rate as u64;

    let duration = if cfg.duration_secs > 0 {
        Some(Duration::from_secs(cfg.duration_secs))
    } else {
        None
    };

    let started_at = Instant::now();

    // Spawn Kafka producer tasks
    for producer_id in 0..N_PRODUCERS {
        let brokers   = brokers.clone();
        let sent      = sent.clone();
        let errors    = errors.clone();
        let running   = running.clone();
        let hist      = hist.clone();
        let tenants   = tenants.clone();
        let scenario  = cfg.scenario.clone();
        // Triage sampling state shared with the producer tasks
        let triage_hist    = triage_hist.clone();
        let triage_samples = triage_samples.clone();
        let triage_pct     = cfg.sampled_triage_pct;
        let orch_url       = orchestrator_url.clone();

        tokio::spawn(async move {
            if let Err(e) = produce_loop(
                producer_id,
                brokers,
                interval_us,
                scenario,
                tenants,
                sent,
                errors,
                running,
                hist,
                triage_pct,
                orch_url,
                triage_hist,
                triage_samples,
            )
            .await
            {
                warn!(producer = producer_id, "loop error: {e}");
            }
        });
    }

    // Duration watcher
    if let Some(dur) = duration {
        let running_w = running.clone();
        let started = started_at;
        tokio::spawn(async move {
            tokio::time::sleep(dur.saturating_sub(started.elapsed())).await;
            running_w.store(false, Ordering::Relaxed);
            info!("load-gen: duration reached, stopping");
        });
    }

    Ok(RunHandle { sent, errors, running, started_at, hist, triage_hist, triage_samples })
}

#[allow(clippy::too_many_arguments)]
async fn produce_loop(
    id: usize,
    brokers: String,
    interval_us: u64,
    scenario: Scenario,
    tenants: Vec<(String, String)>,
    sent: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
    hist: Arc<Mutex<Histogram<u64>>>,
    triage_pct: u8,
    orch_url: Option<String>,
    triage_hist: Arc<Mutex<Histogram<u64>>>,
    triage_samples: Arc<AtomicU64>,
) -> Result<()> {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &brokers)
        .set("linger.ms", "10")
        .set("batch.num.messages", "10000")
        .set("compression.type", "lz4")
        .set("message.timeout.ms", "5000")
        .set("enable.idempotence", "false")
        .create()?;

    // Reusable HTTP client for triage sampling (shared across loop iterations)
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let mut interval = tokio::time::interval(Duration::from_micros(interval_us.max(1)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let n = tenants.len().max(1);
    let mut idx = id % n;
    let mut seq: u64 = 0;

    loop {
        if !running.load(Ordering::Relaxed) { break; }
        interval.tick().await;
        if !running.load(Ordering::Relaxed) { break; }

        let (tenant_id, user_name) = &tenants[idx % tenants.len()];
        idx = (idx + 1) % tenants.len();

        let event = match scenario {
            Scenario::Mixed   => scenarios::pick_event(tenant_id, user_name),
            Scenario::Attack  => scenarios::geo_anomaly(tenant_id, user_name),
            Scenario::Benign  => scenarios::benign_login(tenant_id, user_name),
        };

        let key     = event.tenant_id.clone();
        let payload = match serde_json::to_string(&event) {
            Ok(p) => p,
            Err(e) => { warn!("serialize error: {e}"); errors.fetch_add(1, Ordering::Relaxed); continue; }
        };

        // Kafka publish
        let t0 = Instant::now();
        match producer
            .send(FutureRecord::to(TOPIC).key(&key).payload(&payload), Duration::from_secs(5))
            .await
        {
            Ok(_) => {
                let lat_us = t0.elapsed().as_micros() as u64;
                sent.fetch_add(1, Ordering::Relaxed);
                let _ = hist.lock().map(|mut h| { let _ = h.record(lat_us); });
                debug!(producer = id, "produced event");
            }
            Err((e, _)) => {
                warn!(producer = id, "kafka error: {e}");
                errors.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Optional sampled triage: send N% of events to the orchestrator
        seq += 1;
        if triage_pct > 0 && seq % 100 < triage_pct as u64 {
            if let Some(ref url) = orch_url {
                let url     = format!("{url}/triage");
                let body    = payload.clone();
                let http    = http.clone();
                let th      = triage_hist.clone();
                let ts      = triage_samples.clone();
                tokio::spawn(async move {
                    let t0 = Instant::now();
                    if http.post(&url)
                        .header("Content-Type", "application/json")
                        .body(serde_json::json!({ "alert": serde_json::from_str::<serde_json::Value>(&body).unwrap_or_default() }).to_string())
                        .send()
                        .await
                        .map(|r| r.status().is_success())
                        .unwrap_or(false)
                    {
                        let lat_us = t0.elapsed().as_micros() as u64;
                        ts.fetch_add(1, Ordering::Relaxed);
                        let _ = th.lock().map(|mut h| { let _ = h.record(lat_us); });
                    }
                });
            }
        }
    }

    info!(producer = id, "shutting down");
    Ok(())
}

/// Pre-seed home-region baselines so geo-anomaly detections can fire.
async fn seed_baselines(brokers: &str, tenants: &[(String, String)]) {
    info!("load-gen: seeding baselines for {} tenants", tenants.len());
    let producer: FutureProducer = match ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("message.timeout.ms", "5000")
        .create()
    {
        Ok(p) => p,
        Err(e) => { warn!("seed producer create failed: {e}"); return; }
    };

    let mut count = 0u64;
    for (tenant_id, user_name) in tenants {
        for _ in 0..5u8 {
            let event = scenarios::benign_login(tenant_id, user_name);
            if let Ok(payload) = serde_json::to_string(&event) {
                let _ = producer
                    .send(FutureRecord::to(TOPIC).key(tenant_id.as_str()).payload(&payload), Duration::from_secs(5))
                    .await;
                count += 1;
            }
        }
    }

    info!("load-gen: seeded {count} baseline events — waiting 5s for RisingWave convergence");
    tokio::time::sleep(Duration::from_secs(5)).await;
    info!("load-gen: baseline warm-up complete");
}
