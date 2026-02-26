use axum::middleware;
use axum::routing::get;
use axum::Router;

use super::auth::auth_middleware;
use super::handlers::{get_all_configs, get_single_config, AppState};

/// 创建 API 路由
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/v1/projects/{project}/envs/{env}/configs",
            get(get_all_configs),
        )
        .route(
            "/api/v1/projects/{project}/envs/{env}/configs/{key}",
            get(get_single_config),
        )
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state)
}
