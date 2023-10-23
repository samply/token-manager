use axum::extract::Query;
use axum::response::IntoResponse;
use log::error;
use uuid::Uuid;
use tokio::time::{sleep, Duration};
use crate::utils::{send_email, split_and_trim, generate_token};
use crate::config::CONFIG;
use crate::models::{HttpParams, OpalRequest};

const MAX_RETRIES: u32 = 3;

pub async fn call_opal_api(query: Query<HttpParams>, body: String) -> impl IntoResponse {
    let token = generate_token();
    call_opal_api_with_retries(query, token, body, MAX_RETRIES).await
}

async fn call_opal_api_with_retries(
    query: Query<HttpParams>,
    token: Uuid,
    body: String,
    retries: u32,
) -> impl IntoResponse {
    let mut retries_remaining = retries;
    let mut  response: Result<reqwest::Response, reqwest::Error> = make_opal_request(&query, &token).await;

    while retries_remaining > 0 {
        if response.is_ok() && response.as_ref().unwrap().status().is_success() {
            match send_email(&query, &token, &split_and_trim(&query.projects), &body) {
                Ok(_) => println!("Email sent successfully!"),
                Err(e) => panic!("Could not send email: {:?}", e),
            }
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
        response = make_opal_request(&query, &token).await;
    }

    error!("Request error: {:?}", response.as_ref().unwrap());
    return (hyper::StatusCode::INTERNAL_SERVER_ERROR, format!("Request error: {:?}", response.as_ref().unwrap())).into_response();
}


async fn make_opal_request(query: &Query<HttpParams>, token: &Uuid) -> reqwest::Result<reqwest::Response> {
    let api_url = CONFIG.opal_api_url.clone();
    
    // Splitting from comma-separated string to a Vec<String>
    let bridgeheads = split_and_trim(&query.bridgehead_ids);
    let list_projects = split_and_trim(&query.projects);

    let request = OpalRequest {
        name: query.email.clone(),
        token: token.clone(),
        projects: list_projects.clone(),
    };

    let client = reqwest::Client::new();
    client.post(api_url).json(&request).send().await
}