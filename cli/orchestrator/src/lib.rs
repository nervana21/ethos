#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]
//! Collection of utilities for the Ethos CLI orchestrator.

use thiserror::Error;

/// Errors that can occur during Ethos operations.
#[derive(Debug, Error)]
pub enum CompilerError {
    /// Generic compilation error with a custom message.
    #[error("Compilation error: {0}")]
    Message(String),
}

/// Result type alias for Ethos operations.
pub type Result<T> = std::result::Result<T, CompilerError>;
