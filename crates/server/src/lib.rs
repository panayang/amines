pub mod auth;
pub mod bot;
pub mod db;
pub mod room;
pub mod routes;
pub mod ws;

use auth::AuthService;
use db::Database;
use room::RoomManager;
use routes::create_router;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use ws::AppState;

pub async fn run_server(
    port: u16,
    db_path: &str,
    client_dist: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::new(db_path)?;
    let auth = AuthService::new(db.clone());
    let rooms = RoomManager::new(db.clone());
    let state = Arc::new(AppState { auth, rooms });

    let dist_path = client_dist
        .or_else(|| std::env::var("CLIENT_DIST").ok())
        .or_else(|| {
            if PathBuf::from("dist").exists() {
                Some("dist".to_string())
            } else if PathBuf::from("crates/client/dist").exists() {
                Some("crates/client/dist".to_string())
            } else if let Ok(exe) = std::env::current_exe() {
                let sibling_dist = exe.parent().unwrap_or(&exe).join("dist");
                if sibling_dist.exists() {
                    Some(sibling_dist.to_string_lossy().to_string())
                } else {
                    None
                }
            } else {
                None
            }
        });

    if let Some(ref d) = dist_path {
        tracing::info!("📁 Serving web client static files from: {}", d);
    } else {
        tracing::warn!("⚠️  No web client 'dist' directory found. Running API-only mode.");
    }

    let app = create_router(state, dist_path);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
