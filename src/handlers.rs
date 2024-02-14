use std::io;

use crate::config::BEAM_CLIENT;
use crate::config::CONFIG;
use crate::db::Db;
use crate::enums::{OpalResponse, OpalProjectTablesResponse, OpalProjectStatus, OpalProjectStatusResponse, OpalTokenStatus, OpalRequestType};
use crate::models::{NewToken, OpalRequest, TokenParams, TokensQueryParams, ProjectQueryParams};
use anyhow::Result;
use async_sse::Event;
use axum::http::StatusCode;
use axum::{http::HeaderValue, Json};
use beam_lib::{AppId, TaskRequest, MsgId, TaskResult};
use futures_util::StreamExt;
use futures_util::stream::TryStreamExt;
use chrono::Local;
use reqwest::{header, Method};
use tracing::warn;
use tracing::info;
use serde_json::json;
use uuid::Uuid;

pub async fn send_token_registration_request(db: Db, token_params: TokenParams) -> Result<OpalResponse> {
    let token_name = Uuid::new_v4().to_string();
    let task = create_and_send_task_request(
        OpalRequestType::CREATE,
        Some(token_name.clone()),
        Some(token_params.project_id.to_string()),
        Some(&token_params.bridgehead_ids),
        None, None
    ).await?;
    
    info!("Created token task {task:#?}");
 
    match save_tokens_from_beam(db, task, token_params, token_name).await {
        Ok(response) => Ok(response),
        Err(e) => {
            Err(e)
        }
    }
}

pub async fn send_token_from_db(token_params: TokenParams, token_name: String, token: String){
    let task = create_and_send_task_request(
        OpalRequestType::CREATE,
        Some(token_name.clone()),
        Some(token_params.project_id.to_string()),
        Some(&token_params.bridgehead_ids),
        None, Some(token.clone())
    ).await;
    
    info!("Create token in Opal from DB task: {:?}", task);
    //Ok(())
}

pub async fn remove_project_and_tokens_request(mut db: Db, token_params: ProjectQueryParams) -> Result<OpalProjectStatusResponse>  {
    let task = create_and_send_task_request(
        OpalRequestType::DELETE,
        None,
        Some(token_params.project_id.clone()),
        None,
        Some(token_params.bk), None
    ).await?;

    info!("Remove Project and Token request {task:#?}");

    match remove_project_and_tokens_from_beam(task).await {
        Ok(response) => {
            db.delete_project_db(token_params.project_id);
            Ok(response)
        },
        Err(e) => {
            Err(e)
        }
    }
}

pub async fn remove_tokens_request(mut db: Db, token_params: TokensQueryParams) -> Result<OpalProjectStatusResponse>  {
    let token_name_result = db.get_token_name(token_params.user_id.clone(), token_params.project_id.clone());

    let token_name = match token_name_result {
        Ok(Some(name)) => name,
        Ok(None) => {
            return Err(anyhow::Error::msg("Token not found")) 
        },
        Err(e) => {
            return Err(e.into());
        }
    };


    let task = create_and_send_task_request(
        OpalRequestType::DELETE,
        Some(token_name.clone()),
        None,
        None,
        Some(token_params.bk.clone()), None
    ).await?;

    info!("Remove Tokens request {task:#?}");

    match remove_tokens_from_beam(task).await {
        Ok(response) => {
            db.delete_token_db(token_name);
            Ok(response)
        },
        Err(e) => {
            Err(e)
        }
    }
}

pub async fn refresh_token_request(mut db: Db, token_params: TokenParams) -> Result<OpalResponse> {

    let token_name = match db.get_token_name(token_params.user_id.clone(), token_params.project_id.clone()) {
        Ok(Some(name)) => name,
        Ok(None) => {
            return Err(anyhow::Error::msg("Token name not found")) 
        },
        Err(e) => {
            return Err(e.into());
        }
    };
    
    let token_value = match db.get_token_value(token_params.user_id.clone(), token_params.project_id.clone()) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err(anyhow::Error::msg("Token value not found")) 
        },
        Err(e) => {
            return Err(e.into());
        }
    };

    let task = create_and_send_task_request(
        OpalRequestType::UPDATE,
        Some(token_name.clone()),
        Some(token_params.project_id.clone().to_string()),
        Some(&token_params.bridgehead_ids),
        None, Some(token_value.clone())
    ).await?;

    info!("Refresh token task  {task:#?}");

    match update_tokens_from_beam(db, task, token_params, token_name.clone()).await {
        Ok(response) => Ok(response),
        Err(e) => {
            Err(e)
        }
    }
}

pub async fn fetch_project_tables_names_request(token_params: TokenParams) -> Result<Vec<String>, anyhow::Error> {
    let task = create_and_send_task_request(
        OpalRequestType::SCRIPT,
        Some(token_params.user_id.clone().to_string()),
        Some(token_params.project_id.clone().to_string()),
        Some(&token_params.bridgehead_ids),
        None, None
    ).await?;

    info!("Fetch Project Tables Status  {task:#?}");

    let handle = tokio::task::spawn(fetch_project_tables_from_beam(task));
    let result = handle.await?;
    result
}

pub async fn check_project_status_request(project_id: String, bridgehead: String) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut response_json = json!({
        "project_id": project_id.clone(),
        "bk": bridgehead.clone(),
        "project_status": OpalTokenStatus::NOTFOUND,
    });

    let task =  match create_and_send_task_request(OpalRequestType::STATUS, None, Some(project_id.clone().to_string()), None, Some(bridgehead.clone().to_string()), None).await {
        Ok(result) => result,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Error creating task: {}", e))),
    };

    info!("Check Project Status  {task:#?}");
       
    let project_status_result = match status_from_beam(task).await {
        Ok(response) =>{ 
            info!("Project Status response {response:#?}");
            Ok(response)
        },
        Err(e) => {
            Err(e)
        }
    };

    match project_status_result {
        Ok(OpalProjectStatusResponse::Ok { status }) => {
            response_json["project_status"] = json!(status);
        },
        Ok(OpalProjectStatusResponse::Err { status_code, error }) => {
            let status = StatusCode::from_u16(status_code as u16)
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            eprintln!("Project status error: {}, {}", status, error);
        },
        Err(e) => {
            eprintln!("Error retrieving project status: {:?}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        },
    };
    
    Ok(Json(response_json))
}

pub async fn check_token_status_request(user_id: String, bridgehead: String, project: String, token_name: String, token: String) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut response_json = json!({
        "user_id": user_id.clone(),
        "bk": bridgehead.clone(),
        "token_status": OpalTokenStatus::NOTFOUND,
    });

    let task =  match create_and_send_task_request(OpalRequestType::STATUS, Some(token_name.clone().to_string()), None, None, Some(bridgehead.clone().to_string()), None).await {
        Ok(result) => result,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Error creating task: {}", e))),
    };

    info!("Check Token Status  {task:#?}");
       
    let token_status_result = match status_from_beam(task).await {
        Ok(response) =>{ 
            info!("Token Status response {response:#?}");
            Ok(response)
        },
        Err(e) => {
            Err(e)
        }
    };

    match token_status_result {
        Ok(OpalProjectStatusResponse::Ok { status }) => {
            response_json["token_status"] = json!(status);

            if status == OpalTokenStatus::CREATED.as_str(){
                
                response_json["token_status"] = json!(status);

            }else{

                let params = TokenParams {
                    user_id: user_id.clone(),
                    project_id: project.clone(),
                    bridgehead_ids: vec![bridgehead.clone()]
                };

                send_token_from_db(params,token_name, token).await;  
                response_json["token_status"] = json!(OpalTokenStatus::CREATED.as_str());
            }
        },
        Ok(OpalProjectStatusResponse::Err { status_code, error }) => {
            let status = StatusCode::from_u16(status_code as u16)
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            eprintln!("Token status error: {}, {}", status, error);
        },
        Err(e) => {
            eprintln!("Error retrieving token status: {:?}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        },
    };
    
    Ok(Json(response_json))
}

async fn save_tokens_from_beam(mut db: Db, task: TaskRequest<OpalRequest>, token_params: TokenParams, token_name: String) -> Result<OpalResponse> {
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
                    token_name: &token_name,
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

async fn update_tokens_from_beam(mut db: Db, task: TaskRequest<OpalRequest>, token_params: TokenParams, token_name: String) -> Result<OpalResponse>  {
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
                    token_name: &token_name,
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

async fn status_from_beam(task: TaskRequest<OpalRequest>) -> Result<OpalProjectStatusResponse> {
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
    bridgehead_single: Option<String>,
    token: Option<String> 
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
        token
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
