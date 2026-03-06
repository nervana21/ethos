//! Code-gen: build a thin `TestNode` client with typed-parameter helpers.
//!
//! This module contains the modularized test node generator components,
//! split into logical units for better maintainability and testing.

use ir::RpcDef;
use types::Implementation;

use super::doc_comment::{format_doc_comment, write_doc_comment};
use super::fee_rate_utils::{
    methods_use_amounts_map, methods_use_get_block_template_request, methods_use_sendall_recipient,
};
use crate::utils::{rpc_method_to_rust_name, sanitize_external_identifier, snake_to_pascal_case};
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

        // Whether any param maps to the shared FeeRate type via the type adapter.
        let uses_fee_rate = crate::generators::fee_rate_utils::methods_use_fee_rate(
            methods.iter(),
            type_adapter.as_ref(),
        );

        // Whether any param is FeeRate with name "maxfeerate" (needs custom BTC/kvB serde).
        let uses_maxfeerate = crate::generators::fee_rate_utils::methods_use_maxfeerate(
            methods.iter(),
            type_adapter.as_ref(),
        );
        let uses_sendall_recipient =
            methods_use_sendall_recipient(methods.iter(), type_adapter.as_ref());
        let uses_get_block_template_request =
            methods_use_get_block_template_request(methods.iter(), type_adapter.as_ref());
        let uses_amounts_map = methods_use_amounts_map(methods.iter(), type_adapter.as_ref());
        // Add necessary imports
        if uses_hash_or_height {
            header.push_str("use crate::types::HashOrHeight;\n");
        }
        if uses_public_key {
            header.push_str("use crate::types::PublicKey;\n");
        }
        if uses_fee_rate {
            header.push_str("use crate::types::FeeRate;\n");
        }

        // Serde helper for Option<FeeRate> as maxfeerate (BTC/kvB f64). FeeRate has no built-in Serialize.
        if uses_maxfeerate {
            header.push_str(
                r#"
mod serde_fee_rate {
    pub mod maxfeerate_opt {
        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        use crate::types::FeeRate;

        #[allow(clippy::ref_option)]
        pub fn serialize<S>(f: &Option<FeeRate>, s: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match f {
                Some(r) => s.serialize_some(&((r.to_sat_per_kvb_floor() as f64) / 100_000_000.0)),
                None => s.serialize_none(),
            }
        }

        pub fn deserialize<'d, D: Deserializer<'d>>(d: D) -> Result<Option<FeeRate>, D::Error> {
            let opt: Option<f64> = Option::deserialize(d)?;
            Ok(opt.map(|v| {
                let sat_per_kvb = (v * 100_000_000.0).round().clamp(0.0, u32::MAX as f64) as u32;
                FeeRate::from_sat_per_kvb(sat_per_kvb)
            }))
        }
    }
}

"#,
            );
        }
        // Serde helper for sendmany "amounts": HashMap<Address, Amount> with values as BTC in JSON.
        if uses_amounts_map {
            header.push_str(
                r#"
/// (De)serializes HashMap<Address, Amount> with values as BTC floats (for sendmany "amounts" param).
pub mod serde_amounts_map {
    use std::collections::HashMap;
    use bitcoin::address::NetworkUnchecked;
    use bitcoin::Address;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(
        map: &HashMap<Address<NetworkUnchecked>, bitcoin::Amount>,
        s: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(map.len()))?;
        for (k, v) in map {
            m.serialize_entry(k, &v.to_btc())?;
        }
        m.end()
    }

    pub fn deserialize<'de, D>(
        d: D,
    ) -> Result<HashMap<Address<NetworkUnchecked>, bitcoin::Amount>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = HashMap::<Address<NetworkUnchecked>, f64>::deserialize(d)?;
        map.into_iter()
            .map(|(k, v)| {
                bitcoin::Amount::from_btc(v).map(|a| (k, a)).map_err(serde::de::Error::custom)
            })
            .collect()
    }
}

/// Reference wrapper for the sendmany "amounts" map; serializes to JSON with amounts as BTC.
#[derive(Debug)]
pub struct SendmanyAmountsRef<'a>(pub &'a std::collections::HashMap<bitcoin::Address<bitcoin::address::NetworkUnchecked>, bitcoin::Amount>);

impl serde::Serialize for SendmanyAmountsRef<'_> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde_amounts_map::serialize(self.0, s)
    }
}

"#,
            );
        }

        // Param types that are defined inline so the generated crate stays self-contained.
        if uses_sendall_recipient {
            header.push_str(
                r#"
/// One recipient for the sendall RPC (address and optional amount in BTC).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SendallRecipient {
    /// Destination address (unchecked; use `.assume_checked()` or `.require_network()` when needed).
    pub address: bitcoin::Address<bitcoin::address::NetworkUnchecked>,
    /// Optional amount (omit to send remaining balance). Serialized as BTC in JSON.
    #[serde(default, with = "bitcoin::amount::serde::as_btc::opt")]
    pub amount: Option<bitcoin::Amount>,
}

"#,
            );
        }
        if uses_get_block_template_request {
            header.push_str(
                r#"
/// Request options for the getblocktemplate RPC (mode, capabilities, rules).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GetBlockTemplateRequest {
    /// Optional mode: \"template\", \"proposal\", or omitted.
    pub mode: Option<String>,
    /// Optional list of capability strings.
    pub capabilities: Option<Vec<String>>,
    /// Optional list of rule strings.
    pub rules: Option<Vec<String>>,
}

"#,
            );
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
                let field = sanitize_external_identifier(&p.name);

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
                let ty = if !p.required { format!("Option<{base_ty}>") } else { base_ty.clone() };

                write_doc_comment(&mut code, &p.description, "    ")
                    .expect("Failed to write field doc");
                // FeeRate has no Serialize; use crate serde(with) helpers or bitcoin_units.
                if base_ty == "FeeRate" {
                    let with_path = if p.name == "maxfeerate" {
                        "crate::bitcoin_core_client::params::serde_fee_rate::maxfeerate_opt"
                    } else if p.required {
                        "bitcoin_units::fee_rate::serde::as_sat_per_vb_floor"
                    } else {
                        "bitcoin_units::fee_rate::serde::as_sat_per_vb_floor::opt"
                    };
                    writeln!(code, "    #[serde(with = \"{}\")]", with_path)
                        .expect("Failed to write serde attribute");
                }
                // sendmany "amounts": HashMap<Address, Amount> serializes values as BTC in JSON.
                if p.name == "amounts" && base_ty.contains("HashMap") && base_ty.contains("Amount")
                {
                    writeln!(code, "    #[serde(with = \"crate::bitcoin_core_client::params::serde_amounts_map\")]")
                        .expect("Failed to write serde attribute");
                }
                // Preserve RPC JSON key when Rust field name differs (e.g. minconf -> min_conf)
                if field != p.name {
                    writeln!(code, "    #[serde(rename = \"{}\")]", p.name)
                        .expect("Failed to write serde rename");
                }
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

        // Use implementation-specific test client name (e.g. BitcoinTestClient)
        let client_name = self.implementation.test_client_prefix();

        let client_code = self.generate_combined_client(client_name, &self.version);

        let mod_rs_code = utils::generate_mod_rs(self.implementation.display_name(), client_name);

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
