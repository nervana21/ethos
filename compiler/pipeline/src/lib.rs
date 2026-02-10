#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! High-level pipeline that generates self-contained RPC client crates
//! by orchestrating code generation.
//!
//! This module provides the core functionality for generating client implementations
//! for software that functions within the Bitcoin protocol ecosystem.
//!
//! ## Module Organization
//!
//! The pipeline is organized into focused modules:
//!
//! - `orchestration` - Main pipeline entry points (`run`, `run_all`)
//! - `project_setup` - Project scaffolding and metadata generation
//! - `schema_processing` - Schema loading and normalization
//! - `template_management` - Template file operations
//! - `codegen_orchestration` - High-level code generation coordination
//! - `protocol_compiler` - Protocol compilation logic

use thiserror::Error;

/// Convenient result type for pipeline operations.
pub type Result<T> = std::result::Result<T, PipelineError>;

/// Errors that can occur while running the codegen pipeline.
#[derive(Debug, Error)]
pub enum PipelineError {
    /// Unsupported implementation error.
    #[error("Unsupported implementation: {0}. Supported: BitcoinCore, CoreLightning")]
    UnsupportedImplementation(types::Implementation),
    /// Generic message-based error.
    #[error("{0}")]
    Message(String),
    /// Error from protocol compiler.
    #[error(transparent)]
    ProtocolCompiler(#[from] protocol_compiler::EthosCompilerError),
    /// Error from semantic analysis.
    #[error(transparent)]
    Semantic(#[from] semantics::SemanticError),
    /// Error originating from adapters.
    #[error(transparent)]
    Adapter(#[from] adapters::ProtocolAdapterError),
    /// Error propagated from the codegen crate.
    #[error(transparent)]
    Codegen(#[from] codegen::CodegenError),
    /// I/O error while creating or writing files.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Formatting error when writing out generated code.
    #[error(transparent)]
    Fmt(#[from] std::fmt::Error),
    /// Regex compilation error used during file generation.
    #[error(transparent)]
    Regex(#[from] regex::Error),
}

// Module declarations
pub mod cargo_dependencies;
pub mod codegen_orchestration;
pub mod feature_aware_cargo;
pub mod feature_aware_mod;
pub mod generation_context;
pub mod module_generators;
pub mod orchestration;
pub mod project_setup;
pub mod protocol_compiler;
pub mod template_management;

// Re-export public API from orchestration module
pub use orchestration::{compile_from_ir, prepare_output_dir, run_all};
