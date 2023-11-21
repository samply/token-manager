use axum::{
    extract::{Query, Path},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use crate::models::HttpParams;
use crate::utils::generate_token;
use crate::handlers::save_token_in_opal_app;


async fn create_token(query: Query<HttpParams>)  -> impl IntoResponse {
    let token = generate_token();
    save_token_in_opal_app(query, token).await
}

async fn check_status(Path(project_id): Path<String>) -> impl IntoResponse {
    "check status".to_string()
}

async fn generate_script() -> String {
    "Generate Script".to_string()
}

pub fn configure_routes() -> Router {
    Router::new()
    .route("/createToken", post(create_token))
    .route("/checkStatus/:project_id", get(check_status))
    .route("/generateScript", post(generate_script))
}