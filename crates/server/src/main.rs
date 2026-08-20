use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "amine-server",
    about = "High-Performance WebSocket & HTTP Server for 3D Möbius Minesweeper",
    version
)]
struct Cli {
    /// Port to listen on (defaults to 3500, or $PORT env var)
    #[arg(short, long, env = "PORT", default_value_t = 3500)]
    port: u16,

    /// Host address to bind to (defaults to 0.0.0.0, or $HOST env var)
    #[arg(long, env = "HOST", default_value = "0.0.0.0")]
    host: String,

    /// SQLite database file path (defaults to minesweeper.db, or $DATABASE_PATH env var)
    #[arg(long, env = "DATABASE_PATH", default_value = "minesweeper.db")]
    db: String,

    /// Path to web client static distribution directory (dist)
    #[arg(short, long, env = "CLIENT_DIST")]
    dist: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=info,tower_http=info".into()),
        )
        .init();

    let cli = Cli::parse();

    tracing::info!(
        "🚀 Starting 3D Möbius Minesweeper server on http://{}:{}",
        cli.host,
        cli.port
    );

    let dist_str = cli.dist.map(|p| p.to_string_lossy().to_string());
    server::run_server(cli.port, &cli.db, dist_str).await?;

    Ok(())
}
