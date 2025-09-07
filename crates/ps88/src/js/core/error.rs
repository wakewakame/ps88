use thiserror::Error;

#[derive(Debug, Error)]
pub enum JsRuntimeError {
    #[error("failed to compile: `{0}`")]
    CompileError(String),
    #[error("failed to process: `{0}`")]
    RuntimeError(String),
    #[error("unexpected error: {0}")]
    UnexpectedError(String),
}

pub type Result<T> = std::result::Result<T, JsRuntimeError>;

pub fn wrap_err<T, E: ToString>(e: std::result::Result<T, E>) -> Result<T> {
    match e {
        Ok(v) => Ok(v),
        Err(err) => Err(JsRuntimeError::UnexpectedError(err.to_string())),
    }
}
