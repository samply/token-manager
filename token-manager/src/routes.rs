use axum::{
    extract::{Query, Path, rejection::QueryRejection},
    response::IntoResponse,
    http::StatusCode,
    routing::{get, post},
    Json, Router
};
use serde_json::json;
use crate::models::{HttpParams, ScriptParams};
use crate::utils::generate_token;
use crate::db::{check_project_status, generate_user_script, check_db_status};
use crate::handlers::{save_token_in_opal_app, opal_health_check};

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

async fn create_token(query: Query<HttpParams>)  -> impl IntoResponse {
    let token = generate_token();
    save_token_in_opal_app(query, token).await
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


async fn generate_script(query: Result<Query<ScriptParams>, QueryRejection>) -> impl IntoResponse {
    match query {

        Ok(query) => {

            match generate_user_script(query).await {
                Ok(script) => (StatusCode::OK, Json(script)).into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }

        }
        Err(e) => {
            // If deserialization fails, return an error message
            (StatusCode::BAD_REQUEST, format!("Missing required query parameters: {}", e)).into_response()
        }
    }

}

pub fn configure_routes() -> Router {
    Router::new()
    .route("/health", get(health_check))
    .route("/createToken", post(create_token))
    .route("/checkStatus/:project_id", get(check_status))
    .route("/generateScript", post(generate_script))
}