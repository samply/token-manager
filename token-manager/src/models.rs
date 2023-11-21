use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct HttpParams {
    pub email: String,
    pub projects: String,       // Comma-separated values
    pub bridgehead_ids: String, // Comma-separated values
}

#[derive(Serialize)]
pub struct OpalRequest {
    pub name: String,
    pub token: Uuid,
    pub projects: Vec<String>
}

#[derive(Serialize)]
pub struct TokenManagerResponse {
    pub email: String,
    pub token: Uuid,
    pub projects: Vec<String>,     // projects are a list of strings
    pub bridgeheads: Vec<String>, // bridgeheads are a list of strings
    pub r_script: String,
}

#[derive(Serialize)]
pub struct ProjectRequest {
    pub name: String
}