//! Library root module generator
//!
//! Generates the main lib.rs file for the generated crate.

use std::path::PathBuf;

use super::ModuleGenerator;
use crate::generation_context::GenerationContext;
use crate::PipelineError;

/// Generator for the lib.rs file
pub struct LibRsModuleGenerator;

impl ModuleGenerator for LibRsModuleGenerator {
    fn module_name(&self) -> &str { "lib" }

    fn generate_files(
        &self,
        ctx: &GenerationContext,
    ) -> Result<Vec<(String, String)>, PipelineError> {
        let client_name = ctx.implementation.client_prefix().to_string();

        // Generate implementation-specific node manager and test client names
        let node_manager_name = ctx.implementation.node_manager_name();
        let test_client_name = ctx.implementation.test_client_prefix();

        // Generate clients directory name
        let clients_dir_name = ctx.implementation.client_dir_name();

        // Build bitcoin crate re-exports from the collected external symbol usage
        let bitcoin_symbols = ctx.used_external_symbols.symbols_for_crate("bitcoin");
        // Separate top-level symbols (can go in pub use bitcoin::{...}) from nested paths
        let mut top_level_symbols: Vec<String> = Vec::new();
        let mut nested_paths: Vec<String> = Vec::new();

        for symbol in bitcoin_symbols {
            if symbol.contains("::") {
                // Nested path like "bip32::KeySource" - needs separate pub use statement
                nested_paths.push(symbol);
            } else {
                // Top-level symbol like "Address", "Amount", "ScriptBuf"
                top_level_symbols.push(symbol);
            }
        }

        // Sort for deterministic output
        top_level_symbols.sort();
        nested_paths.sort();

        // Build re-export statements
        let mut re_export_lines = Vec::new();
        if !top_level_symbols.is_empty() {
            re_export_lines.push(format!("pub use bitcoin::{{{}}};", top_level_symbols.join(", ")));
        }
        // Add nested paths as separate re-export statements
        for path in nested_paths {
            re_export_lines.push(format!("pub use bitcoin::{};", path));
        }

        let bitcoin_reexports = re_export_lines.join("\n");

        let node_reexports = format!("pub use node::{{NodeManager, {}}};", node_manager_name);

        let lib_content = format!(
            r#"#![forbid(unsafe_code)]
#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::empty_docs)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(dead_code)]
//! Generated {} RPC client library.
//!
//! This library provides a strongly-typed interface to the {} RPC API.
//! It is generated from the {} RPC API documentation.

// Core modules
pub mod config;
pub mod client_trait;
pub mod node;
pub mod test_config;
pub mod {};
pub mod transport;
pub mod types;

// Re-exports for ergonomic access
pub use config::Config;
pub use client_trait::{};
{}
{}
pub use test_config::TestConfig;
pub use {}::{};
pub use types::*;
pub use transport::{{
    DefaultTransport,
    TransportError,
    RpcClient,
}};
"#,
            ctx.implementation.as_str(),
            ctx.implementation.as_str(),
            ctx.implementation.as_str(),
            clients_dir_name,
            client_name,
            node_reexports,
            bitcoin_reexports,
            clients_dir_name,
            test_client_name
        );

        Ok(vec![("lib.rs".to_string(), lib_content)])
    }

    fn output_subdir(&self, _ctx: &GenerationContext) -> PathBuf { PathBuf::from(".") }

    fn should_generate_mod_rs(&self) -> bool { false }
}
