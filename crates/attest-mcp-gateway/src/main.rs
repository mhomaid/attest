//! MCP Gateway binary — starts the axum HTTP server on `MCP_GATEWAY_PORT` (default 4242).

use attest_mcp_gateway::{build_router, registry::default_registry};
use tokio::net::TcpListener;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let port: u16 = std::env::var("MCP_GATEWAY_PORT")
        .unwrap_or_else(|_| "4242".into())
        .parse()?;

    let registry = default_registry();
    let router = build_router(registry);

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "MCP gateway listening");

    axum::serve(listener, router).await?;
    Ok(())
}
