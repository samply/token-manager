use std::io;

use crate::config::BEAM_CLIENT;
use crate::config::CONFIG;
use crate::db::Db;
use crate::enums::{OpalResponse, OpalProjectTablesResponse, OpalProjectStatus, OpalProjectStatusResponse, OpalTokenStatus, OpalRequestType};
use crate::models::{NewToken, OpalRequest, TokenParams};
use anyhow::Result;
use async_sse::Event;
use axum::http::HeaderValue;
use beam_lib::{AppId, TaskRequest, MsgId, TaskResult};
use futures_util::StreamExt;
use futures_util::stream::TryStreamExt;
use chrono::Local;
use reqwest::{header, Method};
use tracing::warn;
use tracing::info;

pub async fn send_token_registration_request(db: Db, token_params: TokenParams) -> Result<OpalResponse> {
    let task = create_and_send_task_request(
        OpalRequestType::CREATE,
        Some(token_params.user_id.to_string()),
        Some(token_params.project_id.to_string()),
        Some(&token_params.bridgehead_ids),
        None
    ).await?;
    
    info!("Created token task {task:#?}");
 
    match save_tokens_from_beam(db, task, token_params).await {
        Ok(response) => Ok(response),
        Err(e) => {
            Err(e)
        }
    }
}

pub async fn remove_project_and_tokens_request(mut db: Db, project_id: String, bridgehead: String) -> Result<OpalProjectStatusResponse>  {
    let task = create_and_send_task_request(
        OpalRequestType::DELETE,
        None,
        Some(project_id.clone()),
        None,
        Some(bridgehead)
    ).await?;

    info!("Remove Project and Token request {task:#?}");

    match remove_project_and_tokens_from_beam(task).await {
        Ok(response) => {
            db.delete_token_db(project_id);
            Ok(response)
        },
        Err(e) => {
            Err(e)
        }
    }
}

pub async fn remove_tokens_request(mut db: Db, user_id: String, bridgehead: String) -> Result<OpalProjectStatusResponse>  {
    let task = create_and_send_task_request(
        OpalRequestType::DELETE,
        Some(user_id.clone().to_string()),
        None,
        None,
        Some(bridgehead)
    ).await?;

    info!("Remove Tokens request {task:#?}");

    match remove_tokens_from_beam(task).await {
        Ok(response) => {
            db.delete_user_db(user_id);
            Ok(response)
        },
        Err(e) => {
            Err(e)
        }
    }
}

pub async fn refresh_token_request(db: Db, token_params: TokenParams) -> Result<OpalResponse> {
    let task = create_and_send_task_request(
        OpalRequestType::UPDATE,
        Some(token_params.user_id.clone().to_string()),
        Some(token_params.project_id.clone().to_string()),
        Some(&token_params.bridgehead_ids),
        None
    ).await?;

    info!("Refresh token task  {task:#?}");

    match update_tokens_from_beam(db, task, token_params).await {
        Ok(response) => Ok(response),
        Err(e) => {
            Err(e)
        }
    }
}

pub async fn fetch_project_tables_request(token_params: TokenParams) -> Result<Vec<String>, anyhow::Error> {
    let task = create_and_send_task_request(
        OpalRequestType::SCRIPT,
        Some(token_params.user_id.clone().to_string()),
        Some(token_params.project_id.clone().to_string()),
        Some(&token_params.bridgehead_ids),
        None
    ).await?;

    info!("Fetch Project Tables Status  {task:#?}");

    let handle = tokio::task::spawn(fetch_project_tables_from_beam(task));
    let result = handle.await?;
    result
}

pub async fn check_project_status_request(project_id: String, bridgehead: String) -> Result<OpalProjectStatusResponse> {
    let task = create_and_send_task_request(
        OpalRequestType::STATUS,
        None,
        Some(project_id.clone().to_string()),
        None,
        Some(bridgehead)
    ).await?;

    info!("Check Project Status  {task:#?}");
       
    match status_project_from_beam(task).await {
        Ok(response) =>{ 
            info!("Project Status response {response:#?}");
            Ok(response)
        },
        Err(e) => {
            Err(e)
        }
    }
}

async fn save_tokens_from_beam(mut db: Db, task: TaskRequest<OpalRequest>, token_params: TokenParams) -> Result<OpalResponse> {
    let today = Local::now();
    let formatted_date = today.format("%d-%m-%Y").to_string();

    let res = BEAM_CLIENT
        .raw_beam_request(Method::GET, &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()))
        .header(header::ACCEPT, HeaderValue::from_static("text/event-stream"))
        .send()
        .await
        .expect("Beam was reachable in the post request before this");

    let mut stream = async_sse::decode(res.bytes_stream().map_err(|e| io::Error::new(io::ErrorKind::Other, e)).into_async_read());

    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalResponse> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
                continue;
            },
        };

        match result.body {
            OpalResponse::Err { status_code, error } => {
                warn!("{} failed to create a token with status code: {status_code}, error: {error}", result.from);
                return Ok(OpalResponse::Err { status_code, error });
            },
            OpalResponse::Ok { token } => {
                let site_name = result.from.as_ref().split('.').nth(1).expect("Valid app id");
                let new_token = NewToken {
                    token: &token,
                    project_id: &token_params.project_id,
                    bk: &site_name,
                    token_status: OpalTokenStatus::CREATED.as_str(),
                    project_status: OpalProjectStatus::CREATED.as_str(),
                    user_id: &token_params.user_id,
                    token_created_at: &formatted_date,
                };
                db.save_token_db(new_token);
                return Ok(OpalResponse::Ok { token });
            },
        }
    }

    match last_error {
        Some(e) => Err(anyhow::Error::msg(e)),
        None => Err(anyhow::Error::msg("No messages received or processed")), 
    }
}

async fn update_tokens_from_beam(mut db: Db, task: TaskRequest<OpalRequest>, token_params: TokenParams) -> Result<OpalResponse>  {
    let today = Local::now();
    let formatted_date = today.format("%d-%m-%Y").to_string();

    let res = BEAM_CLIENT
        .raw_beam_request(Method::GET, &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()))
        .header(header::ACCEPT, HeaderValue::from_static("text/event-stream"))
        .send()
        .await
        .expect("Beam was reachable in the post request before this");

    let mut stream = async_sse::decode(res.bytes_stream().map_err(|e| io::Error::new(io::ErrorKind::Other, e)).into_async_read());

    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalResponse> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
                continue;
            },
        };

        match result.body {
            OpalResponse::Err { status_code, error } => {
                warn!("{} failed to create a token with status code: {status_code}, error: {error}", result.from);
                return Ok(OpalResponse::Err { status_code, error });
            },
            OpalResponse::Ok { token } => {
                let site_name = result.from.as_ref().split('.').nth(1).expect("Valid app id");
                let new_token = NewToken {
                    token: &token,
                    project_id: &token_params.project_id,
                    bk: &site_name,
                    token_status: OpalTokenStatus::CREATED.as_str(),
                    project_status: OpalProjectStatus::CREATED.as_str(),
                    user_id: &token_params.user_id,
                    token_created_at: &formatted_date,
                };
                db.update_token_db(new_token);
                return Ok(OpalResponse::Ok { token });
            },
        }
    }

    match last_error {
        Some(e) => Err(anyhow::Error::msg(e)),
        None => Err(anyhow::Error::msg("No messages received or processed")), 
    }
}

async fn status_project_from_beam(task: TaskRequest<OpalRequest>) -> Result<OpalProjectStatusResponse> {
    let res = BEAM_CLIENT
        .raw_beam_request(Method::GET, &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()))
        .header(header::ACCEPT, HeaderValue::from_static("text/event-stream"))
        .send()
        .await
        .expect("Beam was reachable in the post request before this");

    let mut stream = async_sse::decode(res.bytes_stream().map_err(|e| io::Error::new(io::ErrorKind::Other, e)).into_async_read());

    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalProjectStatusResponse> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
                continue;
            },
        };

        match result.body {
            OpalProjectStatusResponse::Err { status_code, error } => {
                warn!("{} failed to fecth project status code: {status_code}, error: {error}", result.from);
                return Ok(OpalProjectStatusResponse::Err { status_code, error });
            },
            OpalProjectStatusResponse::Ok { status } => {
                return Ok(OpalProjectStatusResponse::Ok { status });
            },
        }
    }

    match last_error {
        Some(e) => Err(anyhow::Error::msg(e)),
        None => Err(anyhow::Error::msg("No messages received or processed")), 
    }
}

async fn fetch_project_tables_from_beam(task: TaskRequest<OpalRequest>) -> Result<Vec<String>, anyhow::Error> {
    let res = BEAM_CLIENT
        .raw_beam_request(Method::GET, &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()))
        .header(
            header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        )
        .send()
        .await
        .expect("Beam was reachable in the post request before this");
    let mut stream = async_sse::decode(res
        .bytes_stream()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        .into_async_read()
    );
    while let Some(Ok(Event::Message(msg))) =  stream.next().await {
        if msg.name() == "error" {
            warn!("{}", String::from_utf8_lossy(msg.data()));
            break;
        }

        let result: TaskResult<OpalProjectTablesResponse> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to deserialize message {msg:?} into a result: {e}");
                continue;
            },
        };
        
        let tables = &match result.body {
            OpalProjectTablesResponse::Err { error } => {
                warn!("{} failed to update a token: {error}", result.from);
                continue;
            },
            OpalProjectTablesResponse::Ok { tables } => tables,
        };
        
        info!("Check Project Status From Beam {tables:#?}");

        return Ok(tables.clone());
    };
    Err(anyhow::Error::msg("No valid result obtained from the stream"))
}

async fn remove_project_and_tokens_from_beam(task: TaskRequest<OpalRequest>) -> Result<OpalProjectStatusResponse> {
    let res = BEAM_CLIENT
        .raw_beam_request(Method::GET, &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()))
        .header(header::ACCEPT, HeaderValue::from_static("text/event-stream"))
        .send()
        .await
        .expect("Beam was reachable in the post request before this");

    let mut stream = async_sse::decode(res.bytes_stream().map_err(|e| io::Error::new(io::ErrorKind::Other, e)).into_async_read());

    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalProjectStatusResponse> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
                continue;
            },
        };

        match result.body {
            OpalProjectStatusResponse::Err { status_code, error } => {
                warn!("{} failed to fecth project status code: {status_code}, error: {error}", result.from);
                return Ok(OpalProjectStatusResponse::Err { status_code, error });
            },
            OpalProjectStatusResponse::Ok { status } => {
                return Ok(OpalProjectStatusResponse::Ok { status });
            },
        }
    }

    match last_error {
        Some(e) => Err(anyhow::Error::msg(e)),
        None => Err(anyhow::Error::msg("No messages received or processed")), 
    }
}

async fn remove_tokens_from_beam(task: TaskRequest<OpalRequest>) -> Result<OpalProjectStatusResponse> {
    let res = BEAM_CLIENT
        .raw_beam_request(Method::GET, &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()))
        .header(header::ACCEPT, HeaderValue::from_static("text/event-stream"))
        .send()
        .await
        .expect("Beam was reachable in the post request before this");

    let mut stream = async_sse::decode(res.bytes_stream().map_err(|e| io::Error::new(io::ErrorKind::Other, e)).into_async_read());

    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        let result: TaskResult<OpalProjectStatusResponse> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                let error_msg = format!("Failed to deserialize message {msg:?} into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
                continue;
            },
        };

        match result.body {
            OpalProjectStatusResponse::Err { status_code, error } => {
                warn!("{} failed to fecth project status code: {status_code}, error: {error}", result.from);
                return Ok(OpalProjectStatusResponse::Err { status_code, error });
            },
            OpalProjectStatusResponse::Ok { status } => {
                return Ok(OpalProjectStatusResponse::Ok { status });
            },
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
    bridgeheads: Option<&[String]>, 
    bridgehead_single: Option<String>
) -> Result<TaskRequest<OpalRequest>, anyhow::Error> {
    let broker = CONFIG.beam_id.as_ref().splitn(3, '.').nth(2)
        .ok_or_else(|| anyhow::Error::msg("Invalid app id"))?;

    let bks: Vec<_> = if let Some(bridgehead) = bridgehead_single {
        vec![AppId::new_unchecked(format!("{}.{bridgehead}.{broker}", CONFIG.opal_beam_name))]
    } else {
        bridgeheads.unwrap_or(&[]).iter()
            .map(|bk| AppId::new_unchecked(format!("{}.{bk}.{broker}", CONFIG.opal_beam_name)))
            .collect()
    };

    let request = OpalRequest {
        request_type: request_type.to_string(),
        name,
        project,
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

async fn process_beam_response<T: serde::de::DeserializeOwned + 'static>(
    task: &TaskRequest<OpalRequest>
) -> Result<T, anyhow::Error> {
    let res = BEAM_CLIENT
        .raw_beam_request(Method::GET, &format!("/v1/tasks/{}/results?wait_count={}", task.id, task.to.len()))
        .header(header::ACCEPT, HeaderValue::from_static("text/event-stream"))
        .send()
        .await?;

    let mut stream = async_sse::decode(res.bytes_stream().map_err(|e| io::Error::new(io::ErrorKind::Other, e)).into_async_read());
    let mut last_error: Option<String> = None;

    while let Some(Ok(Event::Message(msg))) = stream.next().await {
        match serde_json::from_slice::<TaskResult<T>>(msg.data()) {
            Ok(result) => return Ok(result.body),
            Err(e) => {
                let error_msg = format!("Failed to deserialize message into a result: {e}");
                warn!("{error_msg}");
                last_error = Some(error_msg);
            },
        }
    }

    match last_error {
        Some(e) => Err(anyhow::Error::msg(e)),
        None => Err(anyhow::Error::msg("No messages received or processed")),
    }
}
