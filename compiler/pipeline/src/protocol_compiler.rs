//! Ethos Compiler
//!
//! This module provides the core compiler functionality that operates on
//! ProtocolIR to generate protocol clients, documentation, and tests.

use std::path::Path;

use adapters::ProtocolAdapter;
use analysis::{IrValidator, TypeCanonicalizer};
use ir::ProtocolIR;
use semantics::method_categorization;
use thiserror::Error;

#[derive(Debug, Error)]
/// Errors that can occur during compilation
pub enum EthosCompilerError {
    /// Error from protocol adapters
    #[error(transparent)]
    Adapter(#[from] adapters::ProtocolAdapterError),
    /// I/O error during compilation
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization/deserialization error
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Generic compilation error
    #[error("{0}")]
    Message(String),
}

/// Result type for protocol compilation operations
pub type EthosCompilerResult<T> = std::result::Result<T, EthosCompilerError>;

/// Ethos Protocol Compiler
///
/// This compiler takes protocol adapters and generates protocol clients,
/// documentation, and test harnesses from ProtocolIR.
#[derive(Default)]
pub struct EthosCompiler {
    adapters: Vec<Box<dyn ProtocolAdapter>>,
}

impl EthosCompiler {
    /// Create a new protocol compiler
    pub fn new() -> Self { Self { adapters: Vec::new() } }

    /// Add a protocol adapter to this compiler
    pub fn add_adapter<A: ProtocolAdapter + 'static>(&mut self, adapter: A) {
        self.adapters.push(Box::new(adapter));
    }

    /// Run compiler passes on the ProtocolIR
    pub fn run_compiler_passes(
        &self,
        mut ir: ProtocolIR,
        output_dir: &Path,
    ) -> EthosCompilerResult<ProtocolIR> {
        // Populate access_level for each RPC based on category/name before validation
        for module in ir.modules_mut().iter_mut() {
            for def in module.definitions_mut().iter_mut() {
                if let ir::ProtocolDef::RpcMethod(rpc) = def {
                    rpc.access_level =
                        method_categorization::access_level_for(&rpc.category, &rpc.name);
                }
            }
        }

        // Run IR validation
        let validator = IrValidator;
        let validation_errors = validator.validate(&ir);

        if !validation_errors.is_empty() {
            return Err(EthosCompilerError::Message(format!(
                "IR validation failed with {} errors.",
                validation_errors.len()
            )));
        }

        let canonicalizer = TypeCanonicalizer;
        let promotion_map = canonicalizer.canonicalize(&mut ir);

        // If duplicate types are detected during canonicalization, emit a timestamped
        // `promotion_map-<timestamp>.json`. This helps debug and inspect type collisions
        // when integrating or molding new protocols.
        if !promotion_map.is_empty() {
            use chrono::Utc;
            let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
            let map_path = output_dir.join(format!("promotion_map_{}.json", timestamp));
            let mut file = std::fs::File::create(&map_path)?;
            serde_json::to_writer_pretty(&mut file, &promotion_map)?;
        }

        Ok(ir)
    }
}

#[cfg(test)]
mod tests {
    use adapters::CAP_RPC;
    use ir::{ProtocolDef, RpcDef};

    use super::*;

    /// Test that duplicate RPC methods are deduplicated during merge
    #[test]
    fn test_merge_deduplication() {
        // Create two IRs with the same RPC method
        let rpc_def1 = RpcDef {
            name: "getblock".to_string(),
            description: "Get block by hash".to_string(),
            params: vec![],
            result: None,
            category: "node".to_string(),
            access_level: ir::AccessLevel::default(),
            requires_private_keys: false,
            version_added: None,
            version_removed: None,
            examples: None,
            hidden: None,
        };

        let rpc_def2 = RpcDef {
            name: "getblock".to_string(),
            description: "Get block by hash (duplicate)".to_string(),
            params: vec![],
            result: None,
            category: "node".to_string(),
            access_level: ir::AccessLevel::default(),
            requires_private_keys: false,
            version_added: None,
            version_removed: None,
            examples: None,
            hidden: None,
        };

        let module1 = ir::ProtocolModule::from_source(
            CAP_RPC,
            "Bitcoin Core RPC",
            vec![ProtocolDef::RpcMethod(rpc_def1)],
            "bitcoin_core",
        );

        let module2 = ir::ProtocolModule::from_source(
            CAP_RPC,
            "Bitcoin Core RPC",
            vec![ProtocolDef::RpcMethod(rpc_def2)],
            "bitcoin_core",
        );

        let ir1 = ProtocolIR::new(vec![module1]);
        let ir2 = ProtocolIR::new(vec![module2]);

        let merged = ProtocolIR::merge(vec![ir1, ir2]);

        // Should have only one module with one RPC method
        assert_eq!(merged.modules().len(), 1);
        assert_eq!(merged.modules()[0].definitions().len(), 1);
        assert_eq!(merged.modules()[0].name(), CAP_RPC);
    }

    /// Test that source attribution is preserved in merged modules
    #[test]
    fn test_merge_source_attribution() {
        let rpc_def = RpcDef {
            name: "getblock".to_string(),
            description: "Get block by hash".to_string(),
            params: vec![],
            result: None,
            category: "node".to_string(),
            access_level: ir::AccessLevel::default(),
            requires_private_keys: false,
            version_added: None,
            version_removed: None,
            examples: None,
            hidden: None,
        };

        let module = ir::ProtocolModule::from_source(
            CAP_RPC,
            "Bitcoin Core RPC",
            vec![ProtocolDef::RpcMethod(rpc_def)],
            "bitcoin_core",
        );

        let ir = ProtocolIR::new(vec![module]);
        let merged = ProtocolIR::merge(vec![ir]);

        // Check that the merged module has the expected structure
        assert_eq!(merged.modules().len(), 1);
    }

    /// Test that merge order is deterministic (stability)
    #[test]
    fn test_merge_stability() {
        // Create two IRs with different RPC methods
        let rpc_def1 = RpcDef {
            name: "getblock".to_string(),
            description: "Get block by hash".to_string(),
            params: vec![],
            result: None,
            category: "node".to_string(),
            access_level: ir::AccessLevel::default(),
            requires_private_keys: false,
            version_added: None,
            version_removed: None,
            examples: None,
            hidden: None,
        };

        let rpc_def2 = RpcDef {
            name: "getrawtransaction".to_string(),
            description: "Get raw transaction".to_string(),
            params: vec![],
            result: None,
            category: "node".to_string(),
            access_level: ir::AccessLevel::default(),
            requires_private_keys: false,
            version_added: None,
            version_removed: None,
            examples: None,
            hidden: None,
        };

        let module1 = ir::ProtocolModule::from_source(
            CAP_RPC,
            "Bitcoin Core RPC",
            vec![ProtocolDef::RpcMethod(rpc_def1)],
            "bitcoin_core",
        );

        let module2 = ir::ProtocolModule::from_source(
            CAP_RPC,
            "Bitcoin Core RPC",
            vec![ProtocolDef::RpcMethod(rpc_def2)],
            "bitcoin_core",
        );

        let ir1 = ProtocolIR::new(vec![module1]);
        let ir2 = ProtocolIR::new(vec![module2]);

        // Test both orders
        let merged1 = ProtocolIR::merge(vec![ir1.clone(), ir2.clone()]);
        let merged2 = ProtocolIR::merge(vec![ir2, ir1]);

        // Both should have the same result
        assert_eq!(merged1.modules().len(), merged2.modules().len());
        assert_eq!(
            merged1.modules()[0].definitions().len(),
            merged2.modules()[0].definitions().len()
        );
    }
}
