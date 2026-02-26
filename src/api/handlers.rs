use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;
use tokio::sync::RwLock;

use crate::core::ConfigCenter;
use crate::error::ConfigError;

/// 共享状态类型
pub type AppState = Arc<RwLock<ConfigCenter>>;

// ---- 响应结构体 ----

#[derive(Serialize)]
pub struct AllConfigsResponse {
    pub project: String,
    pub environment: String,
    pub configs: HashMap<String, serde_json::Value>,
}

#[derive(Serialize)]
pub struct SingleConfigResponse {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ---- ConfigError -> HTTP Response ----

impl IntoResponse for ConfigError {
    fn into_response(self) -> Response {
        let status = match &self {
            ConfigError::ProjectNotFound(_) => StatusCode::NOT_FOUND,
            ConfigError::EnvironmentNotFound(_) => StatusCode::NOT_FOUND,
            ConfigError::ConfigItemNotFound(_) => StatusCode::NOT_FOUND,
            ConfigError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ConfigError::Forbidden(_) => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = ErrorResponse {
            error: self.to_string(),
        };
        (status, Json(body)).into_response()
    }
}

// ---- 处理器 ----

/// GET /api/v1/projects/{project}/envs/{env}/configs
pub async fn get_all_configs(
    State(center): State<AppState>,
    Path((project, env)): Path<(String, String)>,
) -> Result<Json<AllConfigsResponse>, ConfigError> {
    let center = center.read().await;
    let configs = center.get_merged_config(&project, &env)?;
    Ok(Json(AllConfigsResponse {
        project,
        environment: env,
        configs,
    }))
}

/// GET /api/v1/projects/{project}/envs/{env}/configs/{key}
pub async fn get_single_config(
    State(center): State<AppState>,
    Path((project, env, key)): Path<(String, String, String)>,
) -> Result<Json<SingleConfigResponse>, ConfigError> {
    let center = center.read().await;
    let value = center.get_merged_config_item(&project, &env, &key)?;
    Ok(Json(SingleConfigResponse { key, value }))
}
