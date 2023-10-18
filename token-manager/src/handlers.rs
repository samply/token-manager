use axum::extract::Query;
use axum::response::IntoResponse;
use log::{error, info};
use uuid::Uuid;
use tokio::time::{sleep, Duration};
use async_recursion::async_recursion;
use crate::utils::{send_email, split_and_trim, generate_token};
use crate::config::CONFIG;
use crate::models::{HttpParams, OpalRequest, TokenManagerResponse};

const MAX_RETRIES: u32 = 3;

pub async fn call_opal_api(query: Query<HttpParams>, body: String) -> impl IntoResponse {
    let token = generate_token();
    call_opal_api_with_retries(query, token, body, MAX_RETRIES).await
}

#[async_recursion]
async fn call_opal_api_with_retries(
    query: Query<HttpParams>,
    token: Uuid,
    body: String,
    retries: u32,
) -> impl IntoResponse {
    match make_opal_request(&query, &token).await {
        Ok(resp) => {
            if resp.status().is_success() {
                match send_email(&query, &token, &split_and_trim(&query.projects), &body) {
                    Ok(_) => println!("Email sent successfully!"),
                    Err(e) => panic!("Could not send email: {:?}", e),
                }
                (hyper::StatusCode::OK, format!("Response successfully: {:?}", resp)).into_response()
            } else {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                error!("Request failed. Status: {}. Error: {}", status, text);
                (hyper::StatusCode::BAD_REQUEST, format!("Request failed. Error: {}", text)).into_response()
            }
        }
        Err(e) if retries > 0 => {
            error!("Request error: {:?}. Retrying (attempt {}/{})", e, MAX_RETRIES - retries + 1, MAX_RETRIES);
            sleep(Duration::from_millis(5000)).await;  // Add a delay before retrying
            call_opal_api_with_retries(query, token, body, retries - 1).await
        }
        Err(e) => {
            error!("Request error: {:?}", e);
            (hyper::StatusCode::INTERNAL_SERVER_ERROR, format!("Request error: {:?}", e)).into_response()
        }
    }
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