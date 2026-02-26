use axum::routing::get;
use axum::Router;

use super::handlers::{export_env, get_all_configs, get_single_config, AppState};

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
        .with_state(state)
}
