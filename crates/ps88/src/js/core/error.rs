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
