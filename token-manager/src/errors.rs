use thiserror::Error;

#[derive(Error, Debug)]
pub enum TokenManagerError {
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Failed while send email confirmation: {0}")]
    FailedSendEmail(String),
}