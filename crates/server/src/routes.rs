use crate::ws::{ws_handler, AppState};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use shared::protocol::{
    AuthRequest, AuthResponse, PbRecordRequest, RoomSummary, UserStatsResponse,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

pub fn create_router(state: Arc<AppState>, client_dist_path: Option<String>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        .route("/auth/register", post(handle_register))
        .route("/auth/login", post(handle_login))
        .route("/auth/me", get(handle_me))
        .route("/stats", get(handle_stats))
        .route("/pb/record", post(handle_record_pb))
        .route("/rooms", get(handle_list_rooms));

    let mut app = Router::new()
        .nest("/api", api_routes)
        .route("/ws", get(ws_handler))
        .layer(cors)
        .with_state(state);

    if let Some(dist) = client_dist_path {
        app = app.fallback_service(ServeDir::new(dist));
    }

    app
}

fn extract_token(headers: &HeaderMap) -> Option<String> {
    let auth_header = headers.get("Authorization")?.to_str().ok()?;
    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        Some(token.trim().to_string())
    } else {
        Some(auth_header.trim().to_string())
    }
}

async fn handle_register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let (token, user) = state
        .auth
        .register(&req.username, &req.password)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(AuthResponse {
        token,
        user_id: user.id,
        username: user.username,
    }))
}

async fn handle_login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let (token, user) = state
        .auth
        .login(&req.username, &req.password)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

    Ok(Json(AuthResponse {
        token,
        user_id: user.id,
        username: user.username,
    }))
}

async fn handle_me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<UserStatsResponse>, (StatusCode, String)> {
    let token =
        extract_token(&headers).ok_or((StatusCode::UNAUTHORIZED, "Missing auth token".into()))?;
    let user_id = state.auth.get_user_id_by_token(&token).ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid or expired session".into(),
    ))?;

    let stats = state
        .auth
        .get_db()
        .get_user_stats(&user_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(stats))
}

async fn handle_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<UserStatsResponse>, (StatusCode, String)> {
    handle_me(State(state), headers).await
}

async fn handle_record_pb(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<PbRecordRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let token =
        extract_token(&headers).ok_or((StatusCode::UNAUTHORIZED, "Missing auth token".into()))?;
    let user_id = state.auth.get_user_id_by_token(&token).ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid or expired session".into(),
    ))?;

    let new_pb = state
        .auth
        .get_db()
        .record_sp_result(
            &user_id,
            req.difficulty,
            &req.config_hash,
            req.time_ms,
            req.moves,
            true,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({
        "new_pb": new_pb,
        "time_ms": req.time_ms
    })))
}

async fn handle_list_rooms(State(state): State<Arc<AppState>>) -> Json<Vec<RoomSummary>> {
    Json(state.rooms.list_rooms())
}
