//! Code-gen: build a thin `TestNode` client with typed-parameter helpers.
//!
//! This module contains the modularized test node generator components,
//! split into logical units for better maintainability and testing.

use ir::RpcDef;
use types::Implementation;

use super::doc_comment::{format_doc_comment, write_doc_comment};
use crate::utils::{rpc_method_to_rust_name, sanitize_field_name, snake_to_pascal_case};
use crate::{CodeGenerator, ProtocolVersion};
pub mod utils;

/// A code generator that creates a protocol-agnostic Rust client library for test environments.
///
/// This generator takes RPC API definitions and produces a complete Rust client library
/// that provides a high-level, type-safe interface for:
/// - Node lifecycle management (start/stop)
/// - Protocol-agnostic RPC method calls
/// - Transport layer abstraction
/// - All RPC methods with proper typing
///
/// The generated client library serves as a test harness that bridges RPC interfaces
/// with Rust's type system, making it easier to write reliable integration tests
/// without dealing with low-level RPC details.
///
/// The generator produces several key components:
/// - Type-safe parameter structs for RPC calls
/// - Type-safe result structs for RPC responses
/// - A high-level test client with dependency injection
/// - Protocol-agnostic node manager interface
///
/// This abstraction layer enables developers to focus on test logic rather than RPC mechanics,
/// while maintaining type safety and proper error handling throughout the test suite.
pub struct TestNodeGenerator {
    version: ProtocolVersion,
    implementation: Implementation,
}

impl TestNodeGenerator {
    /// Creates a new `TestNodeGenerator` configured for a specific version.
    ///
    /// The `version` string determines which RPC methods and structures are used when generating
    /// type-safe test clients and associated modules.
    /// Creates a new `TestNodeGenerator` for the specified version and implementation.
    pub fn new(version: ProtocolVersion, implementation: Implementation) -> Self {
        Self { version, implementation }
    }

    /// Generate params code using the same approach as versioned generators
    fn generate_params_code(&self, methods: &[RpcDef]) -> String {
        let mut header =
            String::from("//! Parameter structs for RPC method calls\nuse serde::Serialize;\n");

        // Get type adapter for mapping protocol types to Rust types
        let type_adapter = self.implementation.create_type_adapter().unwrap_or_else(|_| {
            panic!(
                "Type adapter not available for implementation: {}",
                self.implementation.as_str()
            )
        });

        // Check for custom types that need imports
        let uses_hash_or_height = methods
            .iter()
            .any(|m| m.params.iter().any(|p| p.param_type.name.contains("HashOrHeight")));

        let uses_public_key = methods
            .iter()
            .any(|m| m.params.iter().any(|p| p.param_type.name.contains("PublicKey")));

        let uses_short_channel_id = methods
            .iter()
            .any(|m| m.params.iter().any(|p| p.param_type.name.contains("ShortChannelId")));

        // Add necessary imports
        if uses_hash_or_height {
            header.push_str("use crate::types::HashOrHeight;\n");
        }
        if uses_public_key {
            header.push_str("use crate::types::PublicKey;\n");
        }
        if uses_short_channel_id {
            header.push_str("use crate::types::ShortChannelId;\n");
        }

        header.push('\n');

        let mut code = header;
        for m in methods {
            if m.params.is_empty() {
                continue;
            }
            use std::fmt::Write;
            writeln!(code, "{}", format_doc_comment(&m.description))
                .expect("Failed to write doc comment");
            writeln!(code, "#[derive(Debug, Serialize)]").expect("Failed to write derive");
            writeln!(
                code,
                "pub struct {}Params {{",
                snake_to_pascal_case(&rpc_method_to_rust_name(&m.name))
            )
            .expect("Failed to write struct name");

            for p in &m.params {
                let field = sanitize_field_name(&p.name);

                // Convert param to Argument format and map through type adapter
                let protocol_type = p.param_type.protocol_type.as_ref().unwrap_or_else(|| {
                    panic!(
                        "Parameter '{}' in method '{}' is missing protocol_type. Rust type name is '{}'. \
                        All parameters must have protocol_type set for proper type categorization.",
                        p.name, m.name, p.param_type.name
                    )
                });
                let arg = types::Argument {
                    names: vec![p.name.clone()],
                    type_: protocol_type.clone(),
                    required: p.required,
                    description: p.description.clone(),
                    oneline_description: String::new(),
                    also_positional: false,
                    hidden: false,
                    type_str: None,
                };

                // Map protocol type to Rust type using the adapter
                let (base_ty, _) = types::TypeRegistry::map_argument_type_with_adapter(
                    &arg,
                    type_adapter.as_ref(),
                );
                let ty = if !p.required { format!("Option<{base_ty}>") } else { base_ty };

                write_doc_comment(&mut code, &p.description, "    ")
                    .expect("Failed to write field doc");
                writeln!(code, "    pub {}: {},", field, ty).expect("Failed to write field");
            }
            writeln!(code, "}}\n").expect("Failed to write struct closing");
        }
        code
    }

    /// Generate combined client code
    fn generate_combined_client(&self, client_name: &str, _version: &ProtocolVersion) -> String {
        use std::fmt::Write;
        let mut code = String::new();

        // Generic imports
        writeln!(code, "use std::sync::Arc;").expect("Failed to write import");
        writeln!(code, "use crate::transport::DefaultTransport;").expect("Failed to write import");
        writeln!(code, "use crate::transport::core::TransportTrait;")
            .expect("Failed to write import");

        // Struct definition
        writeln!(code, "#[derive(Debug)]").expect("Failed to write derive");
        writeln!(code, "pub struct {} {{", client_name).expect("Failed to write struct");
        writeln!(code, "    _transport: Arc<DefaultTransport>,").expect("Failed to write field");
        writeln!(code, "}}").expect("Failed to write struct closing");

        // Implementation
        writeln!(code, "impl {} {{", client_name).expect("Failed to write impl");
        writeln!(code, "    pub fn new(transport: Arc<DefaultTransport>) -> Self {{")
            .expect("Failed to write constructor");
        writeln!(code, "        Self {{ _transport: transport }}")
            .expect("Failed to write constructor body");
        writeln!(code, "    }}").expect("Failed to write constructor closing");
        writeln!(code, "    pub fn endpoint(&self) -> &str {{ self._transport.url() }}")
            .expect("Failed to write endpoint accessor");
        writeln!(code, "}}").expect("Failed to write impl closing");

        code
    }
}

impl CodeGenerator for TestNodeGenerator {
    fn generate(&self, methods: &[RpcDef]) -> Vec<(String, String)> {
        // Set the adapter context for method name conversion

        // Use versioned generators for modern approach
        use super::version_specific_client_trait::VersionSpecificClientTraitGenerator;

        // Generate params using the same approach as client trait
        let params_code = self.generate_params_code(methods);

        // Generate client trait using versioned generator
        let client_trait_generator = VersionSpecificClientTraitGenerator::new(
            self.version.clone(),
            self.implementation.as_str().to_string(),
        );

        let client_trait_files = client_trait_generator.generate(methods);

        // Generate protocol-agnostic client name
        let client_name = "TestClient";

        let client_code = self.generate_combined_client(client_name, &self.version);

        let mod_rs_code = utils::generate_mod_rs("test", client_name);

        // Combine all files
        let mut all_files = client_trait_files;
        all_files.push(("params.rs".to_string(), params_code));
        all_files.push(("client.rs".to_string(), client_code));
        all_files.push(("mod.rs".to_string(), mod_rs_code));

        all_files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let version = ProtocolVersion::default();
        let implementation = Implementation::BitcoinCore;
        let generator = TestNodeGenerator::new(version, implementation);

        let files = CodeGenerator::generate(&generator, &[]);
        assert_eq!(files.len(), 5);
    }
}
