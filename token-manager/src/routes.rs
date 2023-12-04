use crate::db::{check_db_status, check_project_status, generate_user_script};
use crate::handlers::{opal_health_check, register_opal_token};
use crate::models::{ScriptParams, TokenParams};
use axum::{
    extract::{rejection::QueryRejection, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

async fn health_check() -> impl IntoResponse {
    // Check database connection
    let database_connection_status = check_db_status().is_ok();

    // Check Opal health
    let opal_health_status = opal_health_check().await.is_ok();

    let status_code = if database_connection_status && opal_health_status {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let response_body = json!({
        "status": if status_code == StatusCode::OK { "ok" } else { "error" },
        "database_connection": if database_connection_status { "ok" } else { "down" },
        "opal_health_check": if opal_health_status { "ok" } else { "down" },
    });

    (status_code, Json(response_body))
}

async fn create_token(
    token_params: Result<Query<TokenParams>, QueryRejection>,
) -> impl IntoResponse {
    match token_params {
        Ok(token_params) => register_opal_token(token_params).await.into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            format!("Missing required token params parameters: {}", e),
        )
            .into_response(),
    }
}

async fn check_status(Path(project_id): Path<String>) -> impl IntoResponse {
    if project_id.is_empty() {
        let error_response = json!({
            "status": "error",
            "message": "Project ID is required"
        });
        return (StatusCode::BAD_REQUEST, Json(error_response)).into_response();
    }

    match check_project_status(project_id).await {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err((status, message)) => (status, Json(json!({"message": message}))).into_response(),
    }
}

async fn generate_script(
    script_params: Result<Query<ScriptParams>, QueryRejection>,
) -> impl IntoResponse {
    match script_params {
        Ok(script_params) => match generate_user_script(script_params).await {
            Ok(script) => (StatusCode::OK, script).into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            format!("Missing required query parameters: {}", e),
        )
            .into_response(),
    }
}

pub fn configure_routes() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/tokens", post(create_token))
        .route("/projects/:project_id/status", get(check_status))
        .route("/scripts", post(generate_script))
}
