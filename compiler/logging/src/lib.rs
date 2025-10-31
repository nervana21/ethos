#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Simple logging utilities for the compiler.

/// Prints a trace message to stderr with module prefix.
pub fn trace(module: &str, msg: &str) {
    eprintln!("[TRACE][{}] {}", module, msg);
}
