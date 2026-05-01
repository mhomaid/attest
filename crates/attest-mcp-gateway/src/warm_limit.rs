//! Rate limiting for `query_warm_tier` MCP tool calls.

use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Inner {
    window_start: Instant,
    count: u32,
}

pub struct WarmTierLimiter {
    inner: Mutex<Inner>,
    max_per_minute: u32,
}

impl WarmTierLimiter {
    pub fn from_env() -> Self {
        let max_per_minute = std::env::var("WARM_TIER_MAX_QPM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(120);
        Self {
            inner: Mutex::new(Inner {
                window_start: Instant::now(),
                count: 0,
            }),
            max_per_minute,
        }
    }

    /// Returns an error when the per-minute budget is exceeded.
    pub fn acquire(&self) -> Result<(), anyhow::Error> {
        let mut g = self.inner.lock().expect("warm tier limiter poisoned");
        let now = Instant::now();
        if now.duration_since(g.window_start) >= Duration::from_secs(60) {
            g.window_start = now;
            g.count = 0;
        }
        if g.count >= self.max_per_minute {
            anyhow::bail!(
                "query_warm_tier rate limit exceeded ({} requests per minute)",
                self.max_per_minute
            );
        }
        g.count += 1;
        Ok(())
    }
}
