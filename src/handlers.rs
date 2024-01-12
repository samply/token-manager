use std::io;

use crate::config::BEAM_CLIENT;
use crate::config::CONFIG;
use crate::db::Db;
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

pub async fn send_token_registration_request(db: Db, token_params: TokenParams) -> Result<()> {
    let bridgeheads = &token_params.bridgehead_ids;
    let broker = CONFIG.beam_id.as_ref().splitn(3, '.').nth(2).expect("Valid app id");
    let bks: Vec<_> = bridgeheads.iter().map(|bk| AppId::new_unchecked(format!("{}.{bk}.{broker}", CONFIG.opal_beam_name))).collect();

    let request = OpalRequest {
        request_type: "CREATE".to_string(),
        name: token_params.email.clone(),
        project: token_params.project_id.clone(),
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
    // TODO: Handle error
    BEAM_CLIENT.post_task(&task).await?;
    info!("Created token task {task:#?}");
    tokio::task::spawn(save_tokens_from_beam(db, task, token_params));
    Ok(())
}

pub async fn remove_project_and_tokens_request(mut db: Db, token_params: TokenParams)-> Result<()> {
    let bridgeheads = &token_params.bridgehead_ids;
    let broker = CONFIG.beam_id.as_ref().splitn(3, '.').nth(2).expect("Valid app id");
    let bks: Vec<_> = bridgeheads.iter().map(|bk| AppId::new_unchecked(format!("{}.{bk}.{broker}", CONFIG.opal_beam_name))).collect();

    let request = OpalRequest {
        request_type: "DELETE".to_string(),
        name: token_params.email.clone(),
        project: token_params.project_id.clone(),
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
    // TODO: Handle error
    BEAM_CLIENT.post_task(&task).await?;
    info!("Remove Project and Token request {task:#?}");
    db.delete_token_db(token_params.email.clone(), token_params.project_id);
    Ok(())
}

pub async fn refresh_token_request(db: Db, token_params: TokenParams) -> Result<()> {
    let bridgeheads = &token_params.bridgehead_ids;
    let broker = CONFIG.beam_id.as_ref().splitn(3, '.').nth(2).expect("Valid app id");
    let bks: Vec<_> = bridgeheads.iter().map(|bk| AppId::new_unchecked(format!("{}.{bk}.{broker}", CONFIG.opal_beam_name))).collect();

    let request = OpalRequest {
        request_type: "UPDATE".to_string(),
        name: token_params.email.clone(),
        project: token_params.project_id.clone(),
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
    // TODO: Handle error
    BEAM_CLIENT.post_task(&task).await?;
    info!("Refresh token task  {task:#?}");
    tokio::task::spawn(update_tokens_from_beam(db, task, token_params));
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum OpalResponse {
    Err {
        error: String,
    },
    Ok {
        token: String,
    }
}

async fn save_tokens_from_beam(mut db: Db, task: TaskRequest<OpalRequest>, token_params: TokenParams) {
    let today = Local::now();
    let formatted_date = today.format("%d-%m-%Y").to_string();
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
        let result: TaskResult<OpalResponse> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to deserialize message {msg:?} into a result: {e}");
                continue;
            },
        };
        let site_name = result.from.as_ref().split('.').nth(1).expect("Valid app id");
        let token = &match result.body {
            OpalResponse::Err { error } => {
                warn!("{} failed to create a token: {error}", result.from);
                continue;
            },
            OpalResponse::Ok { token } => token,
        };
        let new_token = NewToken {
            token,
            project_id: &token_params.project_id,
            bk: &site_name,
            status: "CREATED",
            user_id: &token_params.email,
            created_at: &formatted_date,
        };

        db.save_token_db(new_token);
    };
}

async fn update_tokens_from_beam(mut db: Db, task: TaskRequest<OpalRequest>, token_params: TokenParams) {
    let today = Local::now();
    let formatted_date = today.format("%d-%m-%Y").to_string();
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
        let result: TaskResult<OpalResponse> = match serde_json::from_slice(msg.data()) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to deserialize message {msg:?} into a result: {e}");
                continue;
            },
        };
        let site_name = result.from.as_ref().split('.').nth(1).expect("Valid app id");
        let token = &match result.body {
            OpalResponse::Err { error } => {
                warn!("{} failed to update a token: {error}", result.from);
                continue;
            },
            OpalResponse::Ok { token } => token,
        };
        let new_token = NewToken {
            token,
            project_id: &token_params.project_id,
            bk: &site_name,
            status: "CREATED",
            user_id: &token_params.email,
            created_at: &formatted_date,
        };

        db.update_token_db(new_token);
    };
}