use axum::{
    extract::{Query, Path, rejection::QueryRejection},
    response::{Response, IntoResponse},
    http::StatusCode,
    routing::{get, post},
    Json, Router
};
use hyper::{Body};
use serde_json::json;
use crate::models::{HttpParams, ScriptParams};
use crate::utils::generate_token;
use crate::handlers::{save_token_in_opal_app, establish_connection, opal_health_check, check_project_status, generate_user_script};

async fn health_check() -> Response<Body> {

    // Attempt to establish a database connection
    let database_connection_result = establish_connection();

    // Attempt to call the health_check Python function
    let health_check_result = opal_health_check().await;

    let (status_code, response_body) = if health_check_result.is_ok() {
        (
            hyper::StatusCode::OK,
            json!({
                "status": "ok",
                "database_connection": "ok",
                "health_check": "ok",
            }),
        )
    } else {
        (
            hyper::StatusCode::SERVICE_UNAVAILABLE,
            json!({
                "status": "error",
                "database_connection": "error",
                "health_check": "error",
            }),
        )
    };

    let response_body_str = serde_json::to_string(&response_body).unwrap();

    Response::builder()
        .status(status_code)
        .header("Content-Type", "application/json")
        .body(Body::from(response_body_str))
        .expect("Failed to build response")
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