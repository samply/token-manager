use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OpalResponse<T> {
    Err {
        status_code: i32,
        error_message: String,
    },
    Ok {
        response: T,
    },
}

#[allow(clippy::upper_case_acronyms)]
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
    SCRIPT,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Deserialize, Serialize)]
pub enum OpalProjectStatus {
    #[serde(rename = "CREATED")]
    CREATED,
    #[serde(rename = "WITH_DATA")]
    WITHDATA,
    #[serde(rename = "NOT_FOUND")]
    NOTFOUND,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Deserialize, Serialize)]
pub enum OpalTokenStatus {
    #[serde(rename = "CREATED")]
    CREATED,
    #[serde(rename = "EXPIRED")]
    EXPIRED,
    #[serde(rename = "NOT_FOUND")]
    NOTFOUND,
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

impl fmt::Display for OpalRequestType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            OpalRequestType::CREATE => "CREATE",
            OpalRequestType::DELETE => "DELETE",
            OpalRequestType::UPDATE => "UPDATE",
            OpalRequestType::STATUS => "STATUS",
            OpalRequestType::SCRIPT => "SCRIPT",
        };
        write!(f, "{}", text)
    }
}
