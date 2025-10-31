#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Ethos Intermediate Representation (IR)
//!
//! This crate defines the core IR structures that encapsulate Bitcoin ecosystem
//! protocol specifications at different stages of compilation. The IR serves as the bridge between
//! raw protocol specifications and backend code generation.

pub mod protocol_ir;

// Re-export the main ProtocolIR types for convenience
pub use protocol_ir::*;
