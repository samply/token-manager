use log::error;
use uuid::Uuid;
use tracing::{info, Level};
use tokio::time::{sleep, Duration};
use crate::utils::{split_and_trim, generate_token};
use crate::config::CONFIG;
use crate::models::{HttpParams, OpalRequest, ProjectRequest};
use std::sync::Arc;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

const MAX_RETRIES: u32 = 3;

pub async fn save_token_in_opal_app(
    query: Query<HttpParams>,
    token: Uuid,
) -> impl IntoResponse {
    let mut retries_remaining = MAX_RETRIES;
    let mut  response: Result<reqwest::Response, reqwest::Error> = post_opal_request(&query, &token).await;

    while retries_remaining > 0 {
        if response.is_ok() && response.as_ref().unwrap().status().is_success() {
            return (hyper::StatusCode::OK).into_response();
        }
        else if response.is_ok() && response.as_ref().unwrap().status().is_client_error() {
            let status = response.as_ref().unwrap().status();
            let text = response.as_ref().unwrap();
            error!("Request failed. Status: {}. Error: {:?}", status, text);
            return (hyper::StatusCode::BAD_REQUEST, format!("Request failed. Error: {:?}", text)).into_response();
        }
        if retries_remaining > 1 && response.as_ref().unwrap().status().is_server_error() {
            error!(
                "Request failed. Retrying (attempt {}/{})",
                MAX_RETRIES - retries_remaining + 1,
                MAX_RETRIES
            );
            sleep(Duration::from_millis(5000)).await; // Add a delay before retrying
        }
        retries_remaining -= 1;
        response = post_opal_request(&query, &token).await;
    }

    error!("Request error: {:?}", response.as_ref().unwrap());
    return (hyper::StatusCode::INTERNAL_SERVER_ERROR, format!("Request error: {:?}", response.as_ref().unwrap())).into_response();
}


async fn post_opal_request(query: &Query<HttpParams>, token: &Uuid) -> reqwest::Result<reqwest::Response> {
    let api_url = CONFIG.opal_api_url.clone();
    
    // Splitting from comma-separated string to a Vec<String>
    let bridgeheads = split_and_trim(&query.bridgehead_ids);
    let list_projects = split_and_trim(&query.projects);

    if !list_projects.is_empty() {
        create_project_if_not_exist(list_projects.clone()).await?;
    }

    let request = OpalRequest {
        name: query.email.clone(),
        token: token.clone(),
        projects: list_projects.clone(),
    };

    let client = reqwest::Client::new();
    client.post(api_url + "/token").json(&request).send().await
}

async fn create_project_if_not_exist(projects: Vec<String>) -> reqwest::Result<Vec<reqwest::Response>> {
    let client = reqwest::Client::new();

    let mut responses = Vec::new();

    for project in projects {
        let response = client.get(format!("{}/project/{}", CONFIG.opal_api_url.clone(), project))
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
                    
                }_ => {
                    panic!("Uh oh! Something unexpected happened.");
                }
            }
    }
    Ok(responses)
}

async fn create_project(project: String) -> reqwest::Result<Vec<reqwest::Response>> {
    let client = reqwest::Client::new();

    let request = ProjectRequest {
        name: project.clone()
    };

    let response = client.post(format!("{}/projects", CONFIG.opal_api_url.clone())).json(&request)
        .send()
        .await?;

    let status = response.status();    
    info!("Status of Project {}: {}", project, status.clone());    
    
    Ok(vec![response])
}