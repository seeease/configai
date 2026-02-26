use std::sync::Arc;

use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Json, Response};
use tokio::sync::RwLock;

use super::handlers::ErrorResponse;
use crate::core::ConfigCenter;
use crate::error::ConfigError;

/// 认证中间件：从 X-API-Key 请求头验证 API Key
pub async fn auth_middleware(
    State(center): State<Arc<RwLock<ConfigCenter>>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    // 1. 提取 X-API-Key
    let api_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok());

    let api_key = match api_key {
        Some(k) => k.to_string(),
        None => {
            return Err(error_response(
                StatusCode::UNAUTHORIZED,
                "missing X-API-Key header",
            ));
        }
    };

    // 2. 从 URL 路径提取项目名（第 4 段，index 3）
    //    /api/v1/projects/{project}/envs/{env}/configs[/{key}]
    let path = request.uri().path();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let requested_project = segments.get(3).map(|s| s.to_string());

    // 3. 验证 API Key
    let center = center.read().await;
    match center.validate_api_key(&api_key) {
        Ok(key_info) => {
            // 4. 检查项目匹配
            if let Some(ref project) = requested_project {
                if key_info.project != *project {
                    return Err(error_response(
                        StatusCode::FORBIDDEN,
                        &format!(
                            "api key not authorized for project: {}",
                            project
                        ),
                    ));
                }
            }
        }
        Err(ConfigError::ApiKeyNotFound(_)) => {
            return Err(error_response(
                StatusCode::UNAUTHORIZED,
                "invalid api key",
            ));
        }
        Err(_) => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            ));
        }
    }

    // 释放读锁
    drop(center);

    // 5. 验证通过，继续处理请求
    Ok(next.run(request).await)
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
        .into_response()
}
