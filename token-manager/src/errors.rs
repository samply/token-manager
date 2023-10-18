use thiserror::Error;

#[derive(Error, Debug)]
pub enum FocusError {
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}