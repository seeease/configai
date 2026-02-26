use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde::{Deserialize, Serialize};
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

#[derive(Deserialize, Default)]
pub struct ExportParams {
    #[serde(default)]
    pub prefix: Option<String>,
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
        (
            status,
            Json(ErrorResponse {
                error: self.to_string(),
            }),
        )
            .into_response()
    }
}

// ---- 内联认证 ----

fn validate_request(
    center: &ConfigCenter,
    headers: &HeaderMap,
    project: &str,
) -> Result<(), ConfigError> {
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ConfigError::Unauthorized("missing X-API-Key header".to_string()))?;

    let (key_project, _) = center.validate_api_key(api_key)?;

    if key_project != project {
        return Err(ConfigError::Forbidden(format!(
            "api key not authorized for project: {}",
            project
        )));
    }

    Ok(())
}

// ---- 处理器 ----

/// GET /api/v1/projects/{project}/envs/{env}/configs
pub async fn get_all_configs(
    State(center): State<AppState>,
    headers: HeaderMap,
    Path((project, env)): Path<(String, String)>,
) -> Result<Json<AllConfigsResponse>, ConfigError> {
    let center = center.read().await;
    validate_request(&center, &headers, &project)?;
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
    headers: HeaderMap,
    Path((project, env, key)): Path<(String, String, String)>,
) -> Result<Json<SingleConfigResponse>, ConfigError> {
    let center = center.read().await;
    validate_request(&center, &headers, &project)?;
    let value = center.get_merged_config_item(&project, &env, &key)?;
    Ok(Json(SingleConfigResponse { key, value }))
}

/// GET /api/v1/projects/{project}/envs/{env}/export
pub async fn export_env(
    State(center): State<AppState>,
    headers: HeaderMap,
    Path((project, env)): Path<(String, String)>,
    Query(params): Query<ExportParams>,
) -> Result<String, ConfigError> {
    let center = center.read().await;
    validate_request(&center, &headers, &project)?;
    center.get_env_export(&project, &env, params.prefix.as_deref())
}
