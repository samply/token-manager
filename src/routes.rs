use crate::db::Db;
use crate::handlers::{send_token_registration_request, remove_project_and_tokens_request, refresh_token_request, remove_tokens_request, check_project_status_request};
use crate::enums::{OpalResponse, OpalProjectStatusResponse};
use crate::models::{TokenParams, TokensQueryParams, ProjectStatusQuery};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put, delete},
    Json, Router,
};
use serde_json::json;
use tracing::{warn, error};


async fn create_token(
    db: Db,
    token_params: Json<TokenParams>,
) -> impl IntoResponse {
    match send_token_registration_request(db, token_params.0).await {
        Ok(OpalResponse::Ok { .. }) => {
            StatusCode::OK.into_response()
        },
        Ok(OpalResponse::Err { status_code, error }) => {
            let status = StatusCode::from_u16(status_code as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(json!({ "error": error }))).into_response()
        },
        Err(e) => {
            error!("Unhandled error: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}


async fn check_project_status(
    query: Query<ProjectStatusQuery>,
) -> impl IntoResponse {
    let params = query.0;
    match check_project_status_request(params.project_id, params.bk).await {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err((status, message)) => (status, Json(json!({"message": message}))).into_response(),
    }
}

async fn check_token_status(
    mut db: Db,
    query: Query<TokensQueryParams>,
) -> impl IntoResponse {
    let params = query.0;
    match db.check_token_status(params.user_id, params.bk, params.project_id).await {
        Ok(json) => (StatusCode::OK, json).into_response(),
        Err((status, message)) => (status, Json(json!({"message": message}))).into_response(),
    }
}

async fn generate_script(
    mut db: Db,
    script_params: Json<TokenParams>,
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
    db: Db,
    token_params: Json<TokenParams>,
) -> impl IntoResponse  {
    match refresh_token_request(db, token_params.0).await {
        Ok(OpalResponse::Ok { .. }) => {
            StatusCode::OK.into_response()
        },
        Ok(OpalResponse::Err { status_code, error }) => {
            let status = StatusCode::from_u16(status_code as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(json!({ "error": error }))).into_response()
        },
        Err(e) => {
            error!("Unhandled error: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn remove_project_and_token(db: Db, 
    query: Query<TokensQueryParams>,
    ) -> impl IntoResponse {
    match remove_project_and_tokens_request(db, query.0).await {
        Ok(OpalProjectStatusResponse::Ok { .. }) =>  StatusCode::OK.into_response(), 
        Ok(OpalProjectStatusResponse::Err { status_code, error }) => {
            let status = StatusCode::from_u16(status_code as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(json!({ "error": error }))).into_response()
        },
        Err(e) => {
            error!("Unhandled error: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn remove_tokens(db: Db, 
    query: Query<TokensQueryParams>,
    ) -> impl IntoResponse {
    match remove_tokens_request(db, query.0).await {
        Ok(OpalProjectStatusResponse::Ok { .. }) =>  StatusCode::OK.into_response(), 
        Ok(OpalProjectStatusResponse::Err { status_code, error }) => {
            let status = StatusCode::from_u16(status_code as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(json!({ "error": error }))).into_response()
        },
        Err(e) => {
            error!("Unhandled error: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub fn configure_routes(pool: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::prelude::SqliteConnection>>) -> Router {
    Router::new()
        .route("/token", post(create_token))
        .route("/token", delete(remove_tokens))
        .route("/token-status", get(check_token_status))
        .route("/project-status", get(check_project_status))
        .route("/script", post(generate_script))
        .route("/refreshToken", put(refresh_token))
        .route("/project", delete(remove_project_and_token)) 
        .with_state(pool)
}
