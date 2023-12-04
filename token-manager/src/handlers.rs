use crate::config::CONFIG;
use crate::db::save_token_db;
use crate::models::{NewToken, OpalRequest, ProjectRequest, TokenParams};
use crate::utils::{generate_token, split_and_trim};
use anyhow::Result;
use axum::{extract::Query, response::IntoResponse, Json};
use chrono::Local;
use hyper::StatusCode;
use log::error;
use reqwest::{Client, Response};
use serde_json::json;
use tokio::time::{sleep, Duration};
use tracing::info;

const MAX_RETRIES: u32 = 3;

pub async fn register_opal_token(token_params: Query<TokenParams>) -> impl IntoResponse {
    let mut retries_remaining = MAX_RETRIES;

    while retries_remaining > 0 {
        match send_token_registration_request(&token_params).await {
            Ok(response) if response.status().is_success() => {
                return StatusCode::OK.into_response();
            }

            Ok(response) => {
                if response.status().is_client_error() {
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    error!("Request failed. Status: {}. Error: {}", status, text);
                    return (
                        StatusCode::BAD_REQUEST,
                        format!("Request failed. Error: {}", text),
                    )
                        .into_response();
                }

                if retries_remaining > 1 && response.status().is_server_error() {
                    error!(
                        "Request failed. Retrying (attempt {}/{})",
                        MAX_RETRIES - retries_remaining + 1,
                        MAX_RETRIES
                    );
                    sleep(Duration::from_millis(5000)).await;
                }
            }
            Err(e) => {
                error!("Request error: {:?}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Request error: {:?}", e),
                )
                    .into_response();
            }
        }
        retries_remaining -= 1;
    }
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "All retries failed".to_string(),
    )
        .into_response()
}

async fn send_token_registration_request(token_params: &Query<TokenParams>) -> Result<Response> {
    let bridgeheads = split_and_trim(&token_params.bridgehead_ids);

    if bridgeheads.is_empty() {
        return Err(anyhow::Error::msg("No bridgeheads to process"));
    }

    let client = Client::new();
    let api_url = CONFIG.opal_api_url.clone();
    let today = Local::now();
    let formatted_date = today.format("%d-%m-%Y").to_string();

    let token = generate_token();

    for bridgehead in bridgeheads {
        if !token_params.project_id.is_empty() {
            if let Err(e) = create_project_if_missing(token_params.project_id.clone()).await {
                return Err(anyhow::Error::msg(e));
            }
        }
        let token_str = token.to_string();
        let new_token = NewToken {
            token: &token_str,
            project_id: &token_params.project_id,
            bk: &bridgehead,
            status: "CREATED",
            user_id: &token_params.email,
            created_at: &formatted_date,
        };

        save_token_db(new_token);
    }

    let request = OpalRequest {
        name: token_params.email.clone(),
        token: token.clone(),
        projects: token_params.project_id.clone(),
    };

    match client
        .post(format!("{}/token", api_url))
        .json(&request)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => return Ok(response),
        Ok(response) => return Err(anyhow::Error::msg(response.status())),
        Err(e) => return Err(anyhow::Error::msg(e)),
    }
}

async fn create_project_if_missing(project: String) -> reqwest::Result<reqwest::Response> {
    let client = reqwest::Client::new();

    let response = client
        .get(format!(
            "{}/project/{}",
            CONFIG.opal_api_url.clone(),
            project
        ))
        .send()
        .await?;

    let status = response.status();
    info!("Status of Project {}: {}", project, status.clone());

    match status {
        reqwest::StatusCode::OK => {
            info!("Project Exist!");
        }
        reqwest::StatusCode::NOT_FOUND => {
            info!("Project NOT FOUND, PROCEED TO CREATE IT");
            create_project(project).await?;
        }
        _ => {
            error!("Error while trying to create new Project.");
        }
    }
    Ok(response)
}

async fn create_project(project: String) -> reqwest::Result<Vec<reqwest::Response>> {
    let client = reqwest::Client::new();

    let request = ProjectRequest {
        name: project.clone(),
        title: project.clone(),
    };

    let response = client
        .post(format!("{}/projects", CONFIG.opal_api_url.clone()))
        .json(&request)
        .send()
        .await?;

    let status = response.status();
    info!("Status of Project {}: {}", project, status.clone());

    Ok(vec![response])
}

pub async fn opal_health_check() -> Result<Json<serde_json::Value>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/health", CONFIG.opal_api_url.clone()))
        .send()
        .await?;

    if response.status().is_success() {
        let json_response = json!({
            "status": "success",
            "message": "Opal service is up"
        });
        Ok(Json(json_response))
    } else {
        let error_message = format!(
            "Opal service health check failed with status: {}",
            response.status()
        );
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            error_message,
        )))
    }
}
