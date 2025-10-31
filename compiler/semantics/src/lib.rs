#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Semantic analysis for Bitcoin protocol IR.
//!
//! Builds semantic graphs from protocol IR and validates them against semantic invariants.
//! Tracks relationships between RPC methods and type definitions to ensure protocol correctness.

use ir::{ProtocolIR, TypeKind};

/// Shared method categorization utilities used by pipeline and codegen
pub mod method_categorization;

pub use method_categorization::{
    access_level_for, categorize_method, group_methods_by_category, MethodCategory,
};

/// Errors that can occur during semantic analysis.
#[derive(Debug, thiserror::Error)]
pub enum SemanticError {
    /// Invalid intermediate representation encountered.
    #[error("Invalid IR: {0}")]
    InvalidIr(String),
}

/// Result type for semantic analysis operations.
pub type Result<T> = std::result::Result<T, SemanticError>;

/// A semantic invariant that can be checked against a SemanticGraph
#[derive(Clone)]
pub struct SemanticInvariant {
    /// The name of the invariant.
    pub name: String,
    /// Human-readable description of what the invariant checks.
    pub description: String,
    /// Function that validates the invariant against a semantic graph.
    pub check: fn(&SemanticGraph) -> bool,
}

/// A structured diagnostic emitted by semantic analysis
#[derive(Debug, Clone)]
pub struct SemanticDiagnostic {
    /// Name of the invariant that failed.
    pub invariant: String,
    /// Human-readable error message.
    pub message: String,
    /// List of related entities involved in the diagnostic.
    pub related_entities: Vec<String>,
}

/// A graph representing semantic relationships between protocol entities.
#[derive(Default, Clone)]
pub struct SemanticGraph {
    /// All entities in the semantic graph.
    pub entities: Vec<SemanticEntity>,
    /// All relationships between entities.
    pub relations: Vec<SemanticRelation>,
}

/// A semantic entity in the protocol graph.
#[derive(Default, Clone)]
pub struct SemanticEntity {
    /// The name of the entity.
    pub name: String,
    /// The type of semantic entity.
    pub kind: SemanticKind,
}

/// Types of semantic entities in the protocol.
#[derive(Default, Clone, PartialEq)]
pub enum SemanticKind {
    /// An RPC method definition.
    #[default]
    RpcMethod,
    /// A type definition.
    TypeDef,
}

/// A relationship between two semantic entities.
#[derive(Default, Clone)]
pub struct SemanticRelation {
    /// Source entity name.
    pub from: String,
    /// Target entity name.
    pub to: String,
}

/// Analyzer for building semantic graphs from protocol IR.
pub struct SemanticAnalyzer;

impl SemanticAnalyzer {
    /// Builds a semantic graph from a protocol IR.
    pub fn from_ir(ir: &ProtocolIR) -> Result<SemanticGraph> {
        let mut graph = SemanticGraph::default();

        // Track which entities and relations we have already added to avoid duplicates
        let mut entity_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut relation_pairs: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();

        // Add entities for all RPC methods
        for module in ir.modules() {
            for method in module.get_rpc_methods() {
                if entity_names.insert(method.name.clone()) {
                    graph.entities.push(SemanticEntity {
                        name: method.name.clone(),
                        kind: SemanticKind::RpcMethod,
                    });
                }
            }
        }

        // Add entities for all globally defined TypeDefs (if any)
        for typedef in ir.get_type_definitions() {
            if entity_names.insert(typedef.name.clone()) {
                graph.entities.push(SemanticEntity {
                    name: typedef.name.clone(),
                    kind: SemanticKind::TypeDef,
                });
            }
        }

        // Create relations between RpcMethod and TypeDef for parameter and result types.
        // We only relate to non-primitive types. If such a type wasn't globally defined,
        // we still create a TypeDef entity for it to keep the graph closed.
        for module in ir.modules() {
            for method in module.get_rpc_methods() {
                // Params
                for param in &method.params {
                    let t = &param.param_type;
                    if t.kind != TypeKind::Primitive {
                        if entity_names.insert(t.name.clone()) {
                            graph.entities.push(SemanticEntity {
                                name: t.name.clone(),
                                kind: SemanticKind::TypeDef,
                            });
                        }
                        if relation_pairs.insert((method.name.clone(), t.name.clone())) {
                            graph.relations.push(SemanticRelation {
                                from: method.name.clone(),
                                to: t.name.clone(),
                            });
                        }
                    }
                }

                // Result
                if let Some(ret) = &method.result {
                    if ret.kind != TypeKind::Primitive {
                        if entity_names.insert(ret.name.clone()) {
                            graph.entities.push(SemanticEntity {
                                name: ret.name.clone(),
                                kind: SemanticKind::TypeDef,
                            });
                        }
                        if relation_pairs.insert((method.name.clone(), ret.name.clone())) {
                            graph.relations.push(SemanticRelation {
                                from: method.name.clone(),
                                to: ret.name.clone(),
                            });
                        }
                    }
                }
            }
        }

        Ok(graph)
    }

    /// Returns a set of default semantic invariants
    pub fn default_invariants() -> Vec<SemanticInvariant> {
        vec![SemanticInvariant {
            name: "All relations point to known entities".into(),
            description: "Every relation target must exist as an entity".into(),
            check: |g| g.relations.iter().all(|r| g.entities.iter().any(|e| e.name == r.to)),
        }]
    }
}

impl SemanticGraph {
    /// Validate the graph and return detailed diagnostics
    pub fn diagnostics_for_invariants(
        &self,
        invariants: &[SemanticInvariant],
    ) -> Vec<SemanticDiagnostic> {
        let mut diagnostics = Vec::new();

        for inv in invariants {
            if !(inv.check)(self) {
                let mut related = Vec::new();
                if inv.name.contains("relations") {
                    for r in &self.relations {
                        if !self.entities.iter().any(|e| e.name == r.to) {
                            related.push(format!("{} -> {}", r.from, r.to));
                        }
                    }
                }

                diagnostics.push(SemanticDiagnostic {
                    invariant: inv.name.clone(),
                    message: inv.description.clone(),
                    related_entities: related,
                });
            }
        }

        diagnostics
    }
}
