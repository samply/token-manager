use axum::{
    extract::{Query, Path},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use hyper::{Body, Response};
use serde_json::json;
use crate::models::HttpParams;
use crate::utils::generate_token;
use diesel::{SqliteConnection, SelectableHelper, RunQueryDsl};
use crate::handlers::{save_token_in_opal_app, establish_connection, opal_health_check, check_project_status};

async fn health_check() -> Response<Body> {

    let database_connection_result = establish_connection();
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
    check_project_status(project_id).await;
}

async fn generate_script() -> String {
    "Generate Script".to_string()
}

pub fn configure_routes() -> Router {
    Router::new()
    .route("/health", get(health_check))
    .route("/createToken", post(create_token))
    .route("/checkStatus/:project_id", get(check_status))
    .route("/generateScript", post(generate_script))
}