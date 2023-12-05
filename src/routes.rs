use crate::db::Db;
use crate::handlers::register_opal_token;
use crate::models::{ScriptParams, TokenParams};
use axum::{
    extract::{rejection::QueryRejection, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;


async fn create_token(
    token_params: Result<Query<TokenParams>, QueryRejection>,
    db: Db
) -> impl IntoResponse {
    match token_params {
        Ok(token_params) => register_opal_token(db, token_params.0).await.into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            format!("Missing required token params parameters: {}", e),
        )
            .into_response(),
    }
}

async fn check_status(mut db: Db, Path(project_id): Path<String>) -> impl IntoResponse {
    if project_id.is_empty() {
        let error_response = json!({
            "status": "error",
            "message": "Project ID is required"
        });
        return (StatusCode::BAD_REQUEST, Json(error_response)).into_response();
    }

    match db.check_project_status(project_id).await {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err((status, message)) => (status, Json(json!({"message": message}))).into_response(),
    }
}

async fn generate_script(
    script_params: Result<Query<ScriptParams>, QueryRejection>,
    mut db: Db,
) -> impl IntoResponse {
    match script_params {
        Ok(script_params) => match db.generate_user_script(script_params).await {
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

pub fn configure_routes(pool: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::prelude::SqliteConnection>>) -> Router {
    Router::new()
        .route("/tokens", post(create_token))
        .route("/projects/:project_id/status", get(check_status))
        .route("/scripts", get(generate_script))
        .with_state(pool)
}
