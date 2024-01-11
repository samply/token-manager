use crate::db::Db;
use crate::handlers::send_token_registration_request;
use crate::models::{ScriptParams, TokenParams};
use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tracing::{warn, error};


async fn create_token(
    db: Db,
    token_params: Json<TokenParams>,
) -> StatusCode {
    match send_token_registration_request(db, token_params.0).await {
        Ok(_) => {
            StatusCode::OK
        }
        Err(e) => {
            error!("Error creating token task: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn check_status(mut db: Db, Path((project_id, bk)): Path<(String, String)>) -> impl IntoResponse {
    if project_id.is_empty() {
        let error_response = json!({
            "status": "error",
            "message": "Project ID is required"
        });
        return (StatusCode::BAD_REQUEST, Json(error_response)).into_response();
    }

    match db.check_project_status(project_id, bk).await {
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

async fn refresh_token(
    mut db: Db,
    Path((project_id, bk, user_id)): Path<(String, String, String)>
) -> impl IntoResponse {
    StatusCode::OK
}

async fn recreate_project(
    mut db: Db,
    Path((project_id, bk)): Path<(String, String)>
) -> impl IntoResponse {
    StatusCode::OK
}

async fn remove_project_and_tokens(
    mut db: Db,
    Path(project_id): Path<String>
) -> impl IntoResponse {
    StatusCode::OK
}

pub fn configure_routes(pool: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::prelude::SqliteConnection>>) -> Router {
    Router::new()
        .route("/tokens", post(create_token))
        .route("/projects/:project_id/status/:bk", get(check_status))
        .route("/scripts", get(generate_script))
        .route("/projects/:project_id/refreshToken/:bk/:user_id", post(refresh_token)) // New route
        .route("/projects/:project_id/recreateProject/:bk", post(recreate_project))    // New route
        .route("/projects/:project_id/remove", post(remove_project_and_tokens)) 
        .with_state(pool)
}
