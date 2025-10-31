//! Semantic Analysis
//!
//! Performs semantic analysis on the ProtocolIR to ensure
//! semantic invariants are satisfied. Builds a semantic graph from
//! the IR and validates it against a set of semantic invariants.

use semantics::{SemanticAnalyzer as EthosSemanticAnalyzer, SemanticGraph};

use crate::{CompilerContext, CompilerPhase, PhaseResult};

/// Semantic Analyzer for validating semantic invariants
pub struct SemanticAnalyzer;

impl SemanticAnalyzer {
    /// Create a new semantic analyzer
    pub fn new() -> Self { Self }
}

impl Default for SemanticAnalyzer {
    fn default() -> Self { Self::new() }
}

impl CompilerPhase for SemanticAnalyzer {
    fn name(&self) -> &str { "SemanticAnalyzer" }

    fn description(&self) -> &str {
        "Analyze semantic invariants on the ProtocolIR using semantic analysis"
    }

    fn run(&self, ctx: &mut CompilerContext) -> PhaseResult {
        // Build semantic graph from ProtocolIR
        let graph: SemanticGraph = EthosSemanticAnalyzer::from_ir(&ctx.ir).map_err(|e| {
            crate::PhaseError::Other(format!("Failed to build semantic graph: {}", e))
        })?;

        // Get default invariants
        let invariants = EthosSemanticAnalyzer::default_invariants();

        // Validate invariants and collect diagnostics
        let diagnostics = graph.diagnostics_for_invariants(&invariants);

        if diagnostics.is_empty() {
            Ok(())
        } else {
            for d in &diagnostics {
                ctx.add_error(format!("[{}] {}", d.invariant, d.message));
            }
            Err(crate::PhaseError::Other(format!(
                "Semantic invariant violations: {:?}",
                diagnostics.iter().map(|d| &d.invariant).collect::<Vec<_>>()
            )))
        }
    }
}
