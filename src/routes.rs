use crate::db::Db;
use crate::handlers::register_opal_token;
use crate::models::{ScriptParams, TokenParams};
use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tracing::warn;


async fn create_token(
    db: Db,
    token_params: Json<TokenParams>,
) -> StatusCode {
    register_opal_token(db, token_params.0).await
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
    mut db: Db,
    script_params: Json<ScriptParams>,
) -> impl IntoResponse {
        match db.generate_user_script(script_params.0).await {
            Ok(script) => (StatusCode::OK, script).into_response(),
            Err(e) => {
                warn!("Error generating script: {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            },
        }
}

pub fn configure_routes(pool: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::prelude::SqliteConnection>>) -> Router {
    Router::new()
        .route("/tokens", post(create_token))
        .route("/projects/:project_id/status", get(check_status))
        .route("/scripts", get(generate_script))
        .with_state(pool)
}
