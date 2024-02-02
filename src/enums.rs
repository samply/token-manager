use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OpalResponse {
    Err {
        status_code: i32,
        error: String,
    },
    Ok {
        token: String,
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OpalProjectStatusResponse {
    Err {
        status_code: i32,
        error: String,
    },
    Ok {
        status: String,
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OpalProjectTablesResponse {
    Err {
        error: String,
    },
    Ok {
        tables: Vec<String>,
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum OpalRequestType {
    #[serde(rename = "CREATE")]
    CREATE,
    #[serde(rename = "DELETE")]
    DELETE,
    #[serde(rename = "UPDATE")]
    UPDATE,
    #[serde(rename = "STATUS")]
    STATUS,
    #[serde(rename = "SCRIPT")]
    SCRIPT
}

#[derive(Debug, Deserialize, Serialize)]
pub enum OpalProjectStatus {
    #[serde(rename = "CREATED")]
    CREATED,
    #[serde(rename = "WITH_DATA")]
    WITHDATA,
    #[serde(rename = "NOT_FOUND")]
    NOTFOUND
}


#[derive(Debug, Deserialize, Serialize)]
pub enum OpalTokenStatus {
    #[serde(rename = "CREATED")]
    CREATED,
    #[serde(rename = "EXPIRED")]
    EXPIRED,
    #[serde(rename = "NOT_FOUND")]
    NOTFOUND
}

impl OpalProjectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpalProjectStatus::CREATED => "CREATED",
            OpalProjectStatus::WITHDATA => "WITH_DATA",
            OpalProjectStatus::NOTFOUND => "NOT_FOUND",
        }
    }
}

impl OpalTokenStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpalTokenStatus::CREATED => "CREATED",
            OpalTokenStatus::EXPIRED => "EXPIRED",
            OpalTokenStatus::NOTFOUND => "NOT_FOUND",
        }
    }
}

impl OpalRequestType {
    pub fn to_string(&self) -> String {
        match self {
            OpalRequestType::CREATE => "CREATE".to_string(),
            OpalRequestType::DELETE => "DELETE".to_string(),
            OpalRequestType::UPDATE => "UPDATE".to_string(),
            OpalRequestType::STATUS => "STATUS".to_string(),
            OpalRequestType::SCRIPT => "SCRIPT".to_string(),
        }
    }
}

