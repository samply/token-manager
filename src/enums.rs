use serde::{Deserialize, Serialize};

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum OpalResponse {
    Err {
        error: String,
    },
    Ok {
        token: String,
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum OpalProjectResponse {
    Err {
        error: String,
    },
    Ok {
        tables: Vec<String>,
    }
}