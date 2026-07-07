use thiserror::Error;

#[derive(Debug, Error)]
pub enum JsRuntimeError {
    #[error("failed to compile: `{0}`")]
    Compile(String),
    #[error("failed to process: `{0}`")]
    Runtime(String),
    #[error("unexpected error: {0}")]
    Unexpected(String),
}

pub type Result<T> = std::result::Result<T, JsRuntimeError>;

pub fn wrap_err<T, E: ToString>(e: std::result::Result<T, E>) -> Result<T> {
    match e {
        Ok(v) => Ok(v),
        Err(err) => Err(JsRuntimeError::Unexpected(err.to_string())),
    }
}
