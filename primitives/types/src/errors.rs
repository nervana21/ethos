//! Shared error types for protocol adapters
//!
//! This module defines common error types used across different protocol adapters.

use thiserror::Error;

/// Errors that can occur during JSON schema parsing.
#[derive(Error, Debug, Clone)]
pub enum ParseError {
    /// Schema parsing failed
    #[error("Schema parsing failed: {message}")]
    ParseFailed {
        /// Error message describing what went wrong
        message: String,
    },
}
