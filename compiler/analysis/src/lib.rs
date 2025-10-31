#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Ethos Compiler Analysis and Transformation
//!
//! This crate provides analysis and transformation components for the Ethos compiler.
//! Components include validators, normalizers, analyzers, and canonicalizers that
//! operate on the IR through various stages of compilation.

use std::collections::HashMap;

use ir::ProtocolIR;
use thiserror::Error;
use types::{Implementation, ProtocolVersion};

// Import all analysis components
pub mod canonicalizer;
pub mod differential;
pub mod normalizer;
pub mod semantic;
pub mod validator;

// Re-export analysis types
pub use canonicalizer::TypeCanonicalizer;
pub use differential::DifferentialAnalyzer;
pub use normalizer::IRNormalizer;
pub use semantic::SemanticAnalyzer;
pub use validator::IrValidator;

/// Compiler context containing unified state for all compilation phases
#[derive(Debug)]
pub struct CompilerContext {
    /// Protocol IR - single source of truth
    pub ir: ProtocolIR,
    /// Compiler diagnostics
    pub diagnostics: CompilerDiagnostics,
    /// Output directory (optional for analysis-only phases)
    pub output_dir: Option<String>,
    /// The implementation being compiled
    pub implementation: Implementation,
    /// Protocol version being compiled
    pub version: ProtocolVersion,
    /// Path to the input IR file (for reference/debugging)
    pub ir_source_path: Option<std::path::PathBuf>,
}

impl CompilerContext {
    /// Create a new compiler context
    pub fn new(
        implementation: Implementation,
        version: ProtocolVersion,
        ir_source_path: Option<std::path::PathBuf>,
        output_dir: Option<String>,
    ) -> Self {
        Self {
            ir: ProtocolIR::new(vec![]),
            diagnostics: CompilerDiagnostics::default(),
            implementation,
            version,
            ir_source_path,
            output_dir,
        }
    }

    /// Update Protocol IR
    pub fn update_ir(&mut self, ir: ProtocolIR) { self.ir = ir; }

    /// Add diagnostic warning
    pub fn add_warning(&mut self, warning: String) { self.diagnostics.warnings.push(warning); }

    /// Add diagnostic error
    pub fn add_error(&mut self, error: String) { self.diagnostics.errors.push(error); }

    /// Get output path for a specific component
    pub fn output_path(&self, component: &str) -> Option<String> {
        self.output_dir.as_ref().map(|dir| format!("{}/{}", dir, component))
    }
}

/// Compiler diagnostics
#[derive(Debug, Default, Clone)]
pub struct CompilerDiagnostics {
    /// Total methods processed
    pub total_methods: usize,
    /// Warnings generated
    pub warnings: Vec<String>,
    /// Errors generated
    pub errors: Vec<String>,
    /// Statistics
    pub stats: HashMap<String, usize>,
}

impl CompilerDiagnostics {
    /// Merge another diagnostics report
    pub fn merge(&mut self, other: &CompilerDiagnostics) {
        self.total_methods = other.total_methods;
        self.warnings.extend(other.warnings.clone());
        self.errors.extend(other.errors.clone());
        for (k, v) in &other.stats {
            *self.stats.entry(k.clone()).or_insert(0) += v;
        }
    }
}

#[derive(Debug, Error)]
/// Errors produced by individual compiler phases or during orchestration.
pub enum PhaseError {
    /// I/O failure while reading/writing artifacts.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization/deserialization error.
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    /// Any other phase-specific error surfaced as a message.
    #[error("phase error: {0}")]
    Other(String),
}

/// Result alias for phase execution.
pub type Result<T> = std::result::Result<T, PhaseError>;

/// Result of a compiler phase
/// Empty Ok indicates success; errors carry context.
pub type PhaseResult = Result<()>;

/// Trait for compiler phases (analysis, validation, transformation)
///
/// This trait provides a uniform interface for different compiler components,
/// following the pattern used in LLVM and rustc where analyses and transforms
/// can optionally implement a common interface.
pub trait CompilerPhase {
    /// Name of the phase
    fn name(&self) -> &str;

    /// Description of what this phase does
    fn description(&self) -> &str;

    /// Execute this phase on the compiler context
    fn run(&self, ctx: &mut CompilerContext) -> PhaseResult;
}
