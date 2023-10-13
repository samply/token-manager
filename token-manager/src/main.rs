use std::{
    env,
    net::SocketAddr,
};
use axum::{
    extract::{Query},
    response::IntoResponse,
    routing::post,
    Router,
};
use log::{info, LevelFilter};
use serde::{Deserialize, Serialize};
use env_logger::Builder;
use serde_json;
use uuid::Uuid;
use tokio::time::{sleep, Duration};
use log::error;

const MAX_RETRIES: u32 = 3;

#[derive(Deserialize)]
struct QueryParams {
    email: String,
    projects: String,       // Comma-separated values
    bridgehead_ids: String, // Comma-separated values
}

#[derive(Serialize)]
struct OpalRequest {
    name: String,
    token: Uuid,
    projects: Vec<String>
}

#[derive(Serialize)]
struct TokenManagerResponse {
    email: String,
    token: Uuid,
    projects: Vec<String>,     // projects are a list of strings
    bridgeheads: Vec<String>, // bridgeheads are a list of strings
    r_script: String,
}

#[tokio::main]
async fn main() {
    Builder::new()
    .filter(None, LevelFilter::Info)
    .init();

    info!("Starting server on 0.0.0.0:3030");

    let app = Router::new()
    .route("/api/token", post(call_opal_api));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3030));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn generate_r_script(token: Uuid, projects: Vec<String>, body: String) -> String {
    format!(
        r#"
        library(DSI)
        library(DSOpal)
        library(dsBaseClient)

        token <- "{}"
        projects <- "{}"

        builder <- DSI::newDSLoginBuilder(.silent = FALSE)
        builder$append(server='DockerOpal', url="https://opal:8443/opal/", token=token, table=projects, driver="OpalDriver", options = "list(ssl_verifyhost=0,ssl_verifypeer=0)")
        
        logindata <- builder$build()
        connections <- DSI::datashield.login(logins = logindata, assign = TRUE, symbol = "D")
        
        "{}"
        "#,
        token,
        projects.join(","), body
    )
}

async fn call_opal_api(query: Query<QueryParams>, body: String) -> impl IntoResponse {
    let opal_api_url = env::var("OPAL_API_URL").expect("OPAL_API_URL must be set");
    let token = Uuid::new_v4();   // Generate a new Token
 
    // Splitting the bridgehead_ids from comma-separated string to a Vec<String>
    let bridgeheads: Vec<String> = query.bridgehead_ids.split(',')
        .map(|s| s.trim().to_string())
        .collect();
    
    // Splitting the projects from comma-separated string to a Vec<String>
    let list_projects: Vec<String> = query.projects.split(',')
        .map(|s| s.trim().to_string())
        .collect();

    info!("OPAL_API_URL {}", opal_api_url);
    info!("Reques Body {}", body.clone());

    info!("Request Receive /api/token");

    let request = OpalRequest {
        name: query.email.clone(),
        token: token,
        projects: list_projects.clone(),
    };

    let client = reqwest::Client::new();
    
    let r_script = generate_r_script(token, list_projects.clone(), body.clone());

    let mut retries = 0;

    loop {
        match client.post(opal_api_url.clone()).json(&request).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                let response_data = TokenManagerResponse {
                    email: query.email.clone(),
                    token,
                    projects: list_projects.clone(),
                    bridgeheads: bridgeheads.clone(),
                    r_script: r_script.clone()
                };
                let response_json = serde_json::to_string(&response_data).unwrap();
                (hyper::StatusCode::OK, response_json).into_response();
                break;
            } else {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                error!("Request failed. Status: {}. Error: {}", status, text);
                ( hyper::StatusCode::BAD_REQUEST, format!("Request failed. Error: {}", text)).into_response();
                break;
            }
        },
        Err(e) => {
            if retries < MAX_RETRIES {
                retries += 1;
                error!("Request error: {:?}. Retrying (attempt {}/{})", e, retries, MAX_RETRIES);
                sleep(Duration::from_millis(5000)).await;  // Add a delay before retrying
            } else {
                error!("Request error: {:?}", e);
                (hyper::StatusCode::INTERNAL_SERVER_ERROR, format!("Request error: {:?}", e)).into_response();
                break;
            }
            
            }
        }
    }
}