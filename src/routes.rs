use crate::db::Db;
use crate::enums::OpalResponse;
use crate::handlers::{
    check_project_status_request, refresh_token_request, remove_project_and_tokens_request,
    remove_tokens_request, send_token_registration_request,
};
use crate::models::{ProjectQueryParams, TokenParams, TokensQueryParams};
use axum::{
    extract::Query,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde_json::json;
use tracing::debug;

async fn create_token(db: Db, token_params: Json<TokenParams>) -> impl IntoResponse {
    if let Err(e) = send_token_registration_request(db, token_params.0).await {
        debug!("Unhandled error: {e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::OK
    }
}

async fn check_project_status(status_query: Query<ProjectQueryParams>) -> impl IntoResponse {
    match check_project_status_request(status_query.0).await {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err((status, message)) => (status, Json(json!({"message": message}))).into_response(),
    }
}

async fn check_token_status(
    mut db: Db,
    status_query: Query<TokensQueryParams>,
) -> impl IntoResponse {
    match db.check_token_status(status_query.0).await {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err((status, message)) => (status, Json(json!({"message": message}))).into_response(),
    }
}

async fn check_script_status(mut db: Db, status_params: Json<TokenParams>) -> impl IntoResponse {
    match db.check_script_status(status_params.0).await {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err((status, message)) => (status, Json(json!({"message": message}))).into_response(),
    }
}

async fn generate_script(mut db: Db, script_params: Json<TokenParams>) -> impl IntoResponse {
    match db.generate_user_script(script_params.0).await {
        Ok(script) => (StatusCode::OK, script).into_response(),
        Err(e) => {
            debug!("Error generating script: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn refresh_token(db: Db, token_params: Json<TokenParams>) -> StatusCode {
    if let Err(e) = refresh_token_request(db, token_params.0).await {
        debug!("Unhandled error: {e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::OK
    }
}

async fn remove_project_and_token(db: Db, query: Query<ProjectQueryParams>) -> impl IntoResponse {
    match remove_project_and_tokens_request(db, &query.0).await {
        Ok(OpalResponse::Ok { .. }) => StatusCode::OK.into_response(),
        Ok(OpalResponse::Err {
            status_code,
            error_message,
        }) => {
            debug!(
                ?query,
                ?error_message,
                ?status_code,
                "Got error while project"
            );
            let status = StatusCode::from_u16(status_code as u16)
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(json!({ "error": error_message }))).into_response()
        }
        Err(e) => {
            debug!("Unhandled error: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn remove_tokens(db: Db, query: Query<TokensQueryParams>) -> impl IntoResponse {
    match remove_tokens_request(db, &query.0).await {
        Ok(OpalResponse::Ok { .. }) => StatusCode::OK.into_response(),
        Ok(OpalResponse::Err {
            status_code,
            error_message,
        }) => {
            debug!(
                ?query,
                ?error_message,
                ?status_code,
                "Got error while removing tokens"
            );
            let status = StatusCode::from_u16(status_code as u16)
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(json!({ "error": error_message }))).into_response()
        }
        Err(e) => {
            debug!("Unhandled error: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub fn configure_routes(
    pool: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::prelude::SqliteConnection>>,
) -> Router {
    Router::new()
        .route("/token", post(create_token))
        .route("/token", delete(remove_tokens))
        .route("/token-status", get(check_token_status))
        .route("/project-status", get(check_project_status))
        .route("/script", post(generate_script))
        .route("/refreshToken", put(refresh_token))
        .route("/project", delete(remove_project_and_token))
        .route("/authentication-status", post(check_script_status))
        .with_state(pool)
}
