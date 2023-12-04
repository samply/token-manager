use crate::config::BEAM_CLIENT;
use crate::config::CONFIG;
use crate::db::save_token_db;
use crate::models::{NewToken, OpalRequest, TokenParams};
use anyhow::Result;
use async_sse::Event;
use axum::http::HeaderValue;
use futures::{StreamExt, io::BufReader};
use tokio_util::compat::TokioAsyncReadCompatExt;
use beam_lib::{AppId, TaskRequest, MsgId, TaskResult};
use chrono::Local;
use reqwest::{header, Method, StatusCode};
use tracing::{error, info};

pub async fn register_opal_token(token_params: TokenParams) -> StatusCode {
    match send_token_registration_request(&token_params).await {
        Ok(_) => {
            info!("Created token task {token_params:?}");
            StatusCode::OK
        }
        Err(e) => {
            error!("Error creating token task: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn send_token_registration_request(token_params: &TokenParams) -> Result<()> {
    let bridgeheads = &token_params.bridgehead_ids;
    let broker = CONFIG.beam_id.as_ref().splitn(3, '.').nth(2).expect("Valid app id");
    let bks: Vec<_> = bridgeheads.iter().map(|bk| AppId::new_unchecked(format!("dktk-opal.{bk}.{broker}"))).collect();

    let today = Local::now();
    let formatted_date = today.format("%d-%m-%Y").to_string();

    let request = OpalRequest {
        name: token_params.email.clone(),
        project: token_params.project_id.clone(),
    };
    let task_id = MsgId::new();
    let task = TaskRequest {
        id: task_id,
        from: CONFIG.beam_id.clone(),
        to: bks,
        body: request,
        ttl: "60s".into(),
        failure_strategy: beam_lib::FailureStrategy::Discard,
        metadata: serde_json::Value::Null,
    };
    // TODO: Handle error
    BEAM_CLIENT.post_task(&task).await?;
    let res = BEAM_CLIENT
        .raw_beam_request(Method::GET, &format!("/v1/tasks/{task_id}/results?wait_count={}", task.to.len()))
        .header(
            header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        )
        .send()
        .await?;
    let tokio_stream = res.upgrade().await?.compat();
    let mut stream = async_sse::decode(BufReader::new(tokio_stream));
    while let Some(Ok(Event::Message(msg))) =  stream.next().await {
        let result: TaskResult<String> = serde_json::from_slice(msg.data())?;
        let site_name = result.from.as_ref().split('.').nth(1).expect("Valid app id");
        let new_token = NewToken {
            token: &result.body,
            project_id: &token_params.project_id,
            bk: &site_name,
            status: "CREATED",
            user_id: &token_params.email,
            created_at: &formatted_date,
        };

        save_token_db(new_token);
    };
    Ok(())
}
