use std::collections::{HashMap, HashSet};
use std::io;

use crate::config::BEAM_CLIENT;
use crate::config::CONFIG;
use crate::db::Db;
use crate::enums::{OpalProjectStatus, OpalRequestType, OpalResponse, OpalTokenStatus};
use crate::models::{NewToken, OpalRequest, ProjectQueryParams, TokenParams, TokensQueryParams};
use crate::utils::{decrypt_data, encrypt_data};
use anyhow::Result;
use async_sse::Event;
use axum::http::StatusCode;
use axum::{http::HeaderValue, Json};
use base64::{engine::general_purpose::STANDARD, Engine};
use beam_lib::{AppId, MsgId, TaskRequest, TaskResult};
use chrono::Local;
use futures_util::stream::TryStreamExt;
use futures_util::StreamExt;
use reqwest::{header, Method};
use serde_json::json;
use tracing::warn;
use tracing::{debug, info};
use uuid::Uuid;

pub async fn send_token_registration_request(
    mut db: Db,
    token_params: TokenParams,
) -> Result<(), anyhow::Error> {
    if db.is_token_available(token_params.clone())? {
        return Ok(());
    }

    let token_name = Uuid::new_v4().to_string();
    let task = create_and_send_task_request(
        OpalRequestType::CREATE,
        Some(token_name.clone()),
        Some(token_params.project_id.clone().to_string()),
        Some(token_params.bridgehead_ids.clone()),
        None,
    )
    .await?;

    debug!("Created token task {task:#?}");
    tokio::task::spawn(save_tokens_from_beam(db, task, token_params, token_name));
    Ok(())
}

pub async fn send_token_from_db(token_params: TokenParams, token_name: String, token: String) {
    let task = create_and_send_task_request(
        OpalRequestType::CREATE,
        Some(token_name.clone()),
        Some(token_params.project_id.clone().to_string()),
        Some(token_params.bridgehead_ids.clone()),
        Some(token.clone()),
    )
    .await;
    debug!("Create token in Opal from DB task: {:?}", task);
}

pub async fn remove_project_and_tokens_request(
    mut db: Db,
    token_params: &ProjectQueryParams,
) -> Result<OpalResponse<String>, anyhow::Error> {
    let task = create_and_send_task_request(
        OpalRequestType::DELETE,
        None,
        Some(token_params.project_id.clone()),
        Some(vec![token_params.bk.clone()]),
        None,
    )
    .await?;

    debug!("Remove Project and Token request {task:#?}");

    match remove_project_and_tokens_from_beam(task).await {
        Ok(response) => {
            db.delete_project_db(&token_params.project_id);
            Ok(response)
        }
        Err(e) => Err(e),
    }
}

pub async fn remove_tokens_request(
    mut db: Db,
    token_params: &TokensQueryParams,
) -> Result<OpalResponse<String>, anyhow::Error> {
    let token_name = match db.get_token_name(&token_params) {
        Ok(Some(name)) => name,
        Ok(None) => return Err(anyhow::Error::msg("Token not found")),
        Err(e) => {
            return Err(e.into());
        }
    };

    let task = create_and_send_task_request(
        OpalRequestType::DELETE,
        Some(token_name.clone()),
        None,
        Some(vec![token_params.bk.clone()]),
        None,
    )
    .await?;

    debug!("Remove Tokens request {task:#?}");

    match remove_tokens_from_beam(task).await {
        Ok(response) => {
            db.delete_token_db(token_name);
            Ok(response)
        }
        Err(e) => Err(e),
    }
}

pub async fn refresh_token_request(
    mut db: Db,
    token_params: TokenParams,
) -> Result<(), anyhow::Error> {
    let token_query_params: TokensQueryParams = TokensQueryParams {
        user_id: token_params.user_id.clone(),
        bk: token_params.bridgehead_ids[0].clone(),
        project_id: token_params.project_id.clone(),
    };
    let token_name = match db.get_token_name(&token_query_params) {
        Ok(Some(name)) => name,
        Ok(None) => return Err(anyhow::Error::msg("Token name not found")),
        Err(e) => {
            return Err(e.into());
        }
    };

    let token_value = match db.get_token_value(
        token_params.user_id.clone(),
        token_params.project_id.clone(),
        token_params.bridgehead_ids[0].clone(),
    ) {
        Ok(Some(value)) => decrypt_data(value, &token_name.clone().as_bytes()[..16]),
        Ok(None) => return Err(anyhow::Error::msg("Token value not found")),
        Err(e) => {
            return Err(e.into());
        }
    };

    let task = create_and_send_task_request(
        OpalRequestType::UPDATE,
        Some(token_name.clone()),
        Some(token_params.project_id.clone().to_string()),
        Some(token_params.bridgehead_ids.clone()),
        Some(token_value.clone()),
    )
    .await?;

    tokio::task::spawn(update_tokens_from_beam(
        db,
        task,
        token_params,
        token_name.clone(),
    ));
    Ok(())
}

pub async fn fetch_project_tables_names_request(
    token_params: TokenParams,
) -> Result<HashMap<String, HashSet<String>>, anyhow::Error> {
    let task = create_and_send_task_request(
        OpalRequestType::SCRIPT,
        Some(token_params.user_id.clone().to_string()),
        Some(token_params.project_id.clone().to_string()),
        Some(token_params.bridgehead_ids.clone()),
        None,
    )
    .await?;

    debug!("Fetch Project Tables Status  {task:#?}");

    fetch_project_tables_from_beam(task).await
}

pub async fn check_project_status_request(
    query_params: ProjectQueryParams,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut response_json = json!({
        "project_id": query_params.project_id.clone(),
        "bk": query_params.bk.clone(),
        "project_status": OpalTokenStatus::NOTFOUND,
    });

    let task = match create_and_send_task_request(
        OpalRequestType::STATUS,
        None,
        Some(query_params.project_id.clone().to_string()),
        Some(vec![query_params.bk.clone().to_string()]),
        None,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error creating task: {}", e),
            ))
        }
    };

    debug!("Check Project Status  {task:#?}");

    let project_status_result = match status_from_beam(task).await {
        Ok(response) => Ok(response),
        Err(e) => Err(e),
    };

    match project_status_result {
        Ok(OpalResponse::Ok { response }) => {
            info!("Project Status response: {}", json!(response));
            response_json["project_status"] = json!(response);
        }
        Ok(OpalResponse::Err {
            status_code,
            error_message,
        }) => {
            let status = StatusCode::from_u16(status_code as u16)
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            eprintln!("Project status error: {}, {}", status, error_message);
        }
        Err(e) => {
            eprintln!("Error retrieving project status: {:?}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
    };

    Ok(Json(response_json))
}

pub async fn check_token_status_request(
    user_id: String,
    bridgehead: String,
    project: String,
    token_name: String,
    token: String,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut response_json = json!({
        "user_id": user_id.clone(),
        "bk": bridgehead.clone(),
        "token_status": OpalTokenStatus::NOTFOUND,
    });

    let task = match create_and_send_task_request(
        OpalRequestType::STATUS,
        Some(token_name.clone().to_string()),
        None,
        Some(vec![bridgehead.clone().to_string()]),
        None,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error creating task: {}", e),
            ))
        }
    };

    debug!("Check Token Status  {task:#?}");

    let token_status_result = match status_from_beam(task).await {
        Ok(response) => {
            debug!("Token Status response {response:#?}");
            Ok(response)
        }
        Err(e) => Err(e),
    };

    match token_status_result {
        Ok(OpalResponse::Ok { response }) => {
            response_json["token_status"] = json!(response);

            if response == OpalTokenStatus::CREATED.as_str() {
                response_json["token_status"] = json!(response);
            } else {
                let params = TokenParams {
                    user_id: user_id.clone(),
                    project_id: project.clone(),
                    bridgehead_ids: vec![bridgehead.clone()],
                };

                send_token_from_db(params, token_name, token).await;
                response_json["token_status"] = json!(OpalTokenStatus::CREATED.as_str());
            }
        }
        Ok(OpalResponse::Err {
            status_code,
            error_message,
        }) => {
            let status = StatusCode::from_u16(status_code as u16)
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            eprintln!("Token status error: {}, {}", status, error_message);
        }
        Err(e) => {
            eprintln!("Error retrieving token status: {:?}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
    };

    Ok(Json(response_json))
}

async fn save_tokens_from_beam(
    mut db: Db,
    task: TaskRequest<OpalRequest>,
    token_params: TokenParams,
    token_name: String,
) -> Result<()> {
    let today = Local::now();
    let formatted_date = today.format("%d-%m-%Y %H:%M:%S").to_string();

    let res = BEAM_CLIENT
        .raw_beam_request(
            Method::GET,
            &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()),
        )
        .header(
            header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        )
        .send()
        .await
        .expect("Beam was reachable in the post request before this");

    let mut stream = async_sse::decode(
        res.bytes_stream()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .into_async_read(),
    );

    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalResponse<String>> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
                continue;
            }
        };

        match result.body {
            OpalResponse::Err {
                status_code,
                error_message,
            } => {
                warn!("{} failed to create a token with status code: {status_code}, error: {error_message}", result.from);
                last_error = Some(format!("Error: {error_message}"));
            }
            OpalResponse::Ok { response } => {
                let encryp_token = encrypt_data(
                    response.clone().as_bytes(),
                    &token_name.clone().as_bytes()[..16],
                );
                let token_encoded = STANDARD.encode(encryp_token);
                let site_name = result.from.as_ref();

                let new_token = NewToken {
                    token_name: &token_name,
                    token: &token_encoded,
                    project_id: &token_params.project_id,
                    bk: site_name,
                    token_status: OpalTokenStatus::CREATED.as_str(),
                    project_status: OpalProjectStatus::CREATED.as_str(),
                    user_id: &token_params.user_id,
                    token_created_at: &formatted_date,
                };
                db.save_token_db(new_token);
            }
        }
    }

    if let Some(e) = last_error {
        warn!("Error processing task {}: {}", task.id, e);
    }
    Ok(())
}

async fn update_tokens_from_beam(
    mut db: Db,
    task: TaskRequest<OpalRequest>,
    token_params: TokenParams,
    token_name: String,
) -> Result<()> {
    let today = Local::now();
    let formatted_date = today.format("%d-%m-%Y %H:%M:%S").to_string();

    let res = BEAM_CLIENT
        .raw_beam_request(
            Method::GET,
            &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()),
        )
        .header(
            header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        )
        .send()
        .await
        .expect("Beam was reachable in the post request before this");

    let mut stream = async_sse::decode(
        res.bytes_stream()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .into_async_read(),
    );

    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalResponse<String>> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
                continue;
            }
        };

        match result.body {
            OpalResponse::Err {
                status_code,
                error_message,
            } => {
                warn!("{} failed to create a token with status code: {status_code}, error: {error_message}", result.from);
                last_error = Some(format!("Error: {error_message}"));
            }
            OpalResponse::Ok { response } => {
                let encryp_token = encrypt_data(
                    response.clone().as_bytes(),
                    &token_name.clone().as_bytes()[..16],
                );
                let token_encoded = STANDARD.encode(encryp_token);
                let site_name = result.from.as_ref();

                let new_token = NewToken {
                    token_name: &token_name,
                    token: &token_encoded,
                    project_id: &token_params.project_id,
                    bk: site_name,
                    token_status: OpalTokenStatus::CREATED.as_str(),
                    project_status: OpalProjectStatus::CREATED.as_str(),
                    user_id: &token_params.user_id,
                    token_created_at: &formatted_date,
                };
                db.update_token_db(new_token);
            }
        }
    }

    if let Some(e) = last_error {
        warn!("Error processing task {}: {}", task.id, e);
    }
    Ok(())
}

async fn status_from_beam(
    task: TaskRequest<OpalRequest>,
) -> Result<OpalResponse<String>, anyhow::Error> {
    let res = BEAM_CLIENT
        .raw_beam_request(
            Method::GET,
            &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()),
        )
        .header(
            header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        )
        .send()
        .await
        .expect("Beam was reachable in the post request before this");

    let mut stream = async_sse::decode(
        res.bytes_stream()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .into_async_read(),
    );

    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalResponse<String>> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
                continue;
            }
        };

        match result.body {
            OpalResponse::Err {
                status_code,
                error_message,
            } => {
                warn!(
                    "{} failed to fecth project status code: {status_code}, error: {error_message}",
                    result.from
                );
                return Ok(OpalResponse::Err {
                    status_code,
                    error_message,
                });
            }
            OpalResponse::Ok { response } => {
                return Ok(OpalResponse::Ok { response });
            }
        }
    }

    match last_error {
        Some(e) => Err(anyhow::Error::msg(e)),
        None => Err(anyhow::Error::msg("No messages received or processed")),
    }
}

async fn fetch_project_tables_from_beam(
    task: TaskRequest<OpalRequest>,
) -> Result<HashMap<String, HashSet<String>>, anyhow::Error> {
    let res = BEAM_CLIENT
        .raw_beam_request(
            Method::GET,
            &format!(
                "/v1/tasks/{}/results?wait_count={}&wait_time=30s",
                task.id,
                task.to.len()
            ),
        )
        .header(
            header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        )
        .send()
        .await
        .expect("Beam was reachable in the post request before this");
    let mut stream = async_sse::decode(
        res.bytes_stream()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .into_async_read(),
    );

    let mut tables_per_bridgehead: HashMap<String, HashSet<String>> = HashMap::new();
    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalResponse<Vec<String>>> = match serde_json::from_slice(msg.data())
        {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                continue;
            }
        };

        match result.body {
            OpalResponse::Err {
                status_code,
                error_message,
            } => {
                warn!(
                    "status: {} from bk {} failed to fetch tables: {}",
                    status_code, result.from, error_message
                );
                continue;
            }
            OpalResponse::Ok { response } => {
                let bridgehead_tables = tables_per_bridgehead
                    .entry(result.from.as_ref().to_string())
                    .or_default();
                for table in response {
                    bridgehead_tables.insert(table.clone());
                }
            }
        };
    }

    Ok(tables_per_bridgehead)
}

async fn remove_project_and_tokens_from_beam(
    task: TaskRequest<OpalRequest>,
) -> Result<OpalResponse<String>, anyhow::Error> {
    let res = BEAM_CLIENT
        .raw_beam_request(
            Method::GET,
            &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()),
        )
        .header(
            header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        )
        .send()
        .await
        .expect("Beam was reachable in the post request before this");

    let mut stream = async_sse::decode(
        res.bytes_stream()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .into_async_read(),
    );

    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalResponse<String>> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
                continue;
            }
        };

        match result.body {
            OpalResponse::Err {
                status_code,
                error_message,
            } => {
                warn!(
                    "{} failed to fecth project status code: {status_code}, error: {error_message}",
                    result.from
                );
                return Ok(OpalResponse::Err {
                    status_code,
                    error_message,
                });
            }
            OpalResponse::Ok { response } => {
                return Ok(OpalResponse::Ok { response });
            }
        }
    }

    match last_error {
        Some(e) => Err(anyhow::Error::msg(e)),
        None => Err(anyhow::Error::msg("No messages received or processed")),
    }
}

async fn remove_tokens_from_beam(
    task: TaskRequest<OpalRequest>,
) -> Result<OpalResponse<String>, anyhow::Error> {
    let res = BEAM_CLIENT
        .raw_beam_request(
            Method::GET,
            &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()),
        )
        .header(
            header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        )
        .send()
        .await
        .expect("Beam was reachable in the post request before this");

    let mut stream = async_sse::decode(
        res.bytes_stream()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .into_async_read(),
    );

    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalResponse<String>> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
                continue;
            }
        };

        match result.body {
            OpalResponse::Err {
                status_code,
                error_message,
            } => {
                warn!(
                    "{} failed to fecth project status code: {status_code}, error: {error_message}",
                    result.from
                );
                return Ok(OpalResponse::Err {
                    status_code,
                    error_message,
                });
            }
            OpalResponse::Ok { response } => {
                return Ok(OpalResponse::Ok { response });
            }
        }
    }

    match last_error {
        Some(e) => Err(anyhow::Error::msg(e)),
        None => Err(anyhow::Error::msg("No messages received or processed")),
    }
}

async fn create_and_send_task_request(
    request_type: OpalRequestType,
    name: Option<String>,
    project: Option<String>,
    bridgeheads: Option<Vec<String>>,
    token: Option<String>,
) -> Result<TaskRequest<OpalRequest>, anyhow::Error> {
    let bks: Vec<_> = bridgeheads
        .unwrap_or_default()
        .iter()
        .map(|bridgehead_id| AppId::new_unchecked(bridgehead_id.to_string()))
        .collect();

    let request = OpalRequest {
        request_type: request_type.to_string(),
        name,
        project,
        token,
    };

    let task = TaskRequest {
        id: MsgId::new(),
        from: CONFIG.beam_id.clone(),
        to: bks,
        body: request,
        ttl: "60s".into(),
        failure_strategy: beam_lib::FailureStrategy::Discard,
        metadata: serde_json::Value::Null,
    };

    BEAM_CLIENT.post_task(&task).await?;
    Ok(task)
}
