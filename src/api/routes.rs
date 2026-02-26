use axum::middleware::{self, Next};
use axum::extract::Request;
use axum::routing::get;
use axum::Router;

use super::handlers::{export_env, get_all_configs, get_single_config, AppState};

async fn debug_logger(req: Request, next: Next) -> impl axum::response::IntoResponse {
    let method = req.method().clone();
    let uri = req.uri().clone();
    tracing::info!("[DEBUG] --> {} {}", method, uri);
    let response = next.run(req).await;
    tracing::info!("[DEBUG] <-- {} {} => {}", method, uri, response.status());
    response
}

/// 创建 API 路由
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route(
            "/api/v1/projects/{project}/envs/{env}/configs",
            get(get_all_configs),
        )
        .route(
            "/api/v1/projects/{project}/envs/{env}/configs/{key}",
            get(get_single_config),
        )
        .route(
            "/api/v1/projects/{project}/envs/{env}/export",
            get(export_env),
        )
        .layer(middleware::from_fn(debug_logger))
        .with_state(state)
}
