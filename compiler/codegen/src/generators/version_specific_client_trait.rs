//! Version-specific client trait generator
//!
//! This module enhances the client trait generator to use version-specific
//! type metadata for generating accurate method signatures and parameter types.

use std::fmt::Write as _;

use ir::RpcDef;
use types::type_adapter::TypeAdapter;
use types::{Implementation, ProtocolVersion, TypeRegistry};

use super::doc_comment::{format_doc_comment, write_doc_line};
use crate::generators::version_specific_response_type::record_external_symbol_usage;
use crate::utils::{protocol_rpc_method_to_rust_name, sanitize_external_identifier, snake_to_pascal_case};
use crate::CodeGenerator;

/// Enhanced client trait generator that uses version-specific metadata
pub struct VersionSpecificClientTraitGenerator {
    version: ProtocolVersion,
    protocol: Implementation,
}

impl VersionSpecificClientTraitGenerator {
    /// Create a new version-specific client trait generator
    pub fn new(version: ProtocolVersion, protocol: impl Into<Implementation>) -> Self {
        Self { version, protocol: protocol.into() }
    }

    /// Get the protocol adapter for type mapping
    fn get_adapter(&self) -> Box<dyn TypeAdapter> {
        self.protocol.create_type_adapter().unwrap_or_else(|_| {
            panic!("No adapter available for protocol: {}", self.protocol.as_str())
        })
    }
}

impl CodeGenerator for VersionSpecificClientTraitGenerator {
    fn generate(&self, methods: &[RpcDef]) -> Vec<(String, String)> {
        // Filter methods that are available in this version
        let available_methods: Vec<&RpcDef> = methods.iter().collect();

        // render client_trait.rs
        let template = match self.protocol.as_str() {
            "core_lightning" => {
                include_str!("../../templates/core_lightning/client_trait.rs")
            }
            "bitcoin_core" => {
                include_str!("../../templates/bitcoin_core/client_trait.rs")
            }
            _ => panic!("Unsupported protocol: {}", self.protocol),
        };
        let client_trait = self.render_client_trait(template, &available_methods);

        // render mod.rs that re-exports the trait
        let version_no = self.version.as_version_module_name().replace('v', "V");
        let client_name = match self.protocol.as_str() {
            "core_lightning" => "CoreLightningClient",
            "bitcoin_core" => "BitcoinClient",
            _ => panic!("Unsupported protocol: {}", self.protocol),
        };
        let mod_rs = format!(
            "//! Auto-generated module for {client_name}{version_no}\n\
             //!\n\
             //! Generated for Bitcoin Core {}\n\
             //!\n\
             //! This module contains version-specific method signatures that may\n\
             //! not be compatible with other Bitcoin Core versions.\n\
             pub mod client;\n\
             pub use self::client::{client_name}{version_no};\n",
            self.version.as_str()
        );

        vec![("client.rs".into(), client_trait), ("mod.rs".into(), mod_rs)]
    }
}

impl VersionSpecificClientTraitGenerator {
    // Removed filter_methods_for_version as it's no longer needed with RpcDef

    /// Render the client trait with version-specific information
    fn render_client_trait(&self, template: &str, methods: &[&RpcDef]) -> String {
        let mut out = template.to_owned();

        let version_str = self.version.as_str();
        let version_no = self.version.as_version_module_name().replace('v', "V");
        out = out.replace("{{VERSION}}", version_str);
        out = out.replace("{{VERSION_NODOTS}}", &version_no);

        out = out.replace("{{IMPORTS}}", &self.build_imports(methods));

        // No longer generating parameter structs - using individual parameters instead
        out = out.replace("{{PARAM_STRUCTS}}", "");

        let trait_method_signatures = methods
            .iter()
            .map(|m| self.render_method_signature(m))
            .collect::<Vec<_>>()
            .join("\n\n");
        out = out.replace("{{TRAIT_METHOD_SIGNATURES}}", trait_method_signatures.trim_end());

        let trait_method_implementations = methods
            .iter()
            .map(|m| self.render_method_implementation(m))
            .collect::<Vec<_>>()
            .join("\n\n");
        out = out
            .replace("{{TRAIT_METHOD_IMPLEMENTATIONS}}", trait_method_implementations.trim_end());
        out
    }

    /// Build imports for the generated trait
    fn build_imports(&self, methods: &[&RpcDef]) -> String {
        let mut imports =
            vec!["use crate::types::*".to_string(), "use std::future::Future".to_string()];

        // Check for custom types that need imports
        let uses_hash_or_height = methods
            .iter()
            .any(|m| m.params.iter().any(|arg| arg.param_type.name.contains("HashOrHeight")));

        let uses_public_key = methods
            .iter()
            .any(|m| m.params.iter().any(|arg| arg.param_type.name.contains("PublicKey")));

        let uses_short_channel_id = methods
            .iter()
            .any(|m| m.params.iter().any(|arg| arg.param_type.name.contains("ShortChannelId")));

        // Add necessary imports
        if uses_hash_or_height {
            imports.push("use crate::types::HashOrHeight".to_string());
        }
        if uses_public_key {
            // Prefer the canonical bitcoin crate path to avoid duplicate type re-exports
            imports.push("use bitcoin::PublicKey".to_string());
            // Record external symbol usage so lib.rs can re-export it
            record_external_symbol_usage("bitcoin", "PublicKey");
        }
        if uses_short_channel_id {
            imports.push("use crate::types::ShortChannelId".to_string());
        }

        // Avoid adding comment lines or serde imports that may be unused

        // Ensure each import ends with a semicolon
        let mut out = imports.join(";\n");
        out.push_str(";\n");
        out
    }

    /// Render a single method signature for the trait definition
    fn render_method_signature(&self, rpc: &RpcDef) -> String {
        let method_name = protocol_rpc_method_to_rust_name(self.protocol.as_str(), &rpc.name)
            .unwrap_or_else(|e| panic!("{}", e));
        let response_type = self.get_response_type(rpc);

        // Generate individual parameters instead of struct
        let params_sig = if rpc.params.is_empty() {
            "".to_string()
        } else {
            let arguments: Vec<types::Argument> = rpc
                .params
                .iter()
                .map(|param| {
                    let param_type = &param.param_type;
                    let protocol_type = param_type.protocol_type.as_ref().unwrap_or_else(|| {
                        panic!(
							"Parameter '{}' in method '{}' is missing protocol_type. Rust type name is '{}'. \
							All parameters must have protocol_type set for proper type categorization.",
							param.name, rpc.name, param_type.name
						)
                    });
                    types::Argument {
                        names: vec![param.name.clone()],
                        type_: protocol_type.clone(),
                        required: param.required,
                        description: param.description.clone(),
                        oneline_description: String::new(),
                        also_positional: false,
                        hidden: false,
                        type_str: None,
                    }
                })
                .collect();

            let adapter = self.get_adapter();
            let param_parts: Vec<String> = arguments
                .iter()
                .map(|arg| {
                    let param_name = sanitize_external_identifier(&arg.names[0]);
                    // Use protocol adapter to map parameter types in a protocol-agnostic way
                    let (base_ty, _) =
                        TypeRegistry::map_argument_type_with_adapter(arg, adapter.as_ref());
                    // Record external symbol usage if the adapter mapped to a bitcoin type
                    if let Some(stripped) = base_ty.strip_prefix("bitcoin::") {
                        // Take the last path segment as the symbol (e.g., Address, Amount)
                        let symbol = stripped.split("::").last().unwrap_or(stripped);
                        record_external_symbol_usage("bitcoin", symbol);
                    }
                    let param_type = if !arg.required {
                        format!("Option<{}>", base_ty)
                    } else {
                        base_ty.to_string()
                    };
                    format!("{}: {}", param_name, param_type)
                })
                .collect();
            format!(", {}", param_parts.join(", "))
        };

        let mut buf = String::new();

        // Add method documentation
        let formatted_desc = format_doc_comment(&rpc.description);
        for line in formatted_desc.lines() {
            writeln!(buf, "    {}", line).expect("Failed to write method documentation");
        }
        writeln!(buf, "    ///").expect("Failed to write documentation");
        write_doc_line(
            &mut buf,
            &format!("**Version**: Bitcoin Core {}", self.version.as_str()),
            "    ",
        )
        .expect("Failed to write version documentation");

        // Generate method signature (trait definition)
        writeln!(
            buf,
            "    async fn {}(&self{}) -> Result<{}, Self::Error>;",
            method_name, params_sig, response_type
        )
        .expect("Failed to write method signature");

        buf
    }

    /// Render a single method implementation
    fn render_method_implementation(&self, rpc: &RpcDef) -> String {
        let method_name = protocol_rpc_method_to_rust_name(self.protocol.as_str(), &rpc.name)
            .unwrap_or_else(|e| panic!("{}", e));
        let response_type = self.get_response_type(rpc);

        // Generate individual parameters instead of struct
        let params_sig = if rpc.params.is_empty() {
            "".to_string()
        } else {
            let arguments: Vec<types::Argument> = rpc
                .params
                .iter()
                .map(|param| {
                    let param_type = &param.param_type;
                    let protocol_type = param_type.protocol_type.as_ref().unwrap_or_else(|| {
                        panic!(
							"Parameter '{}' in method '{}' is missing protocol_type. Rust type name is '{}'. \
							All parameters must have protocol_type set for proper type categorization.",
							param.name, rpc.name, param_type.name
						)
                    });
                    types::Argument {
                        names: vec![param.name.clone()],
                        type_: protocol_type.clone(),
                        required: param.required,
                        description: param.description.clone(),
                        oneline_description: String::new(),
                        also_positional: false,
                        hidden: false,
                        type_str: None,
                    }
                })
                .collect();

            let adapter = self.get_adapter();
            let param_parts: Vec<String> = arguments
                .iter()
                .map(|arg| {
                    let param_name = sanitize_external_identifier(&arg.names[0]);
                    // Use protocol adapter to map parameter types in a protocol-agnostic way
                    let (base_ty, _) =
                        TypeRegistry::map_argument_type_with_adapter(arg, adapter.as_ref());
                    let param_type = if !arg.required {
                        format!("Option<{}>", base_ty)
                    } else {
                        base_ty.to_string()
                    };
                    format!("{}: {}", param_name, param_type)
                })
                .collect();
            format!(", {}", param_parts.join(", "))
        };

        let mut buf = String::new();

        // Add method documentation
        let formatted_desc = format_doc_comment(&rpc.description);
        for line in formatted_desc.lines() {
            writeln!(buf, "    {}", line).expect("Failed to write method documentation");
        }
        writeln!(buf, "    ///").expect("Failed to write documentation");
        write_doc_line(
            &mut buf,
            &format!("**Version**: Bitcoin Core {}", self.version.as_str()),
            "    ",
        )
        .expect("Failed to write version documentation");

        // Generate method implementation
        writeln!(
            buf,
            "    async fn {}(&self{}) -> Result<{}, Self::Error> {{",
            method_name, params_sig, response_type
        )
        .expect("Failed to write method signature");

        // Add method body - delegate to the transport layer
        // Generate individual parameter serialization
        if !rpc.params.is_empty() {
            // Create params array from individual parameters
            // Optional parameters (Option<T>) should only be included if they're Some(...)
            // Use rpc_params as variable name to avoid conflict with parameter named "params"
            writeln!(buf, "        let mut rpc_params = vec![];")
                .expect("Failed to write params array initialization");
            for param in &rpc.params {
                let param_name = sanitize_external_identifier(&param.name);
                if !param.required {
                    // Optional parameter: only include if Some
                    writeln!(buf, "        if let Some(val) = {} {{", param_name)
                        .expect("Failed to write optional parameter check");
                    writeln!(buf, "            rpc_params.push(serde_json::json!(val));")
                        .expect("Failed to write optional parameter push");
                    writeln!(buf, "        }}")
                        .expect("Failed to write optional parameter closing");
                } else {
                    // Required parameter: always include
                    writeln!(
                        buf,
                        "        rpc_params.push(serde_json::json!({}));",
                        param_name
                    )
                    .expect("Failed to write required parameter serialization");
                }
            }
            writeln!(
                buf,
                "        self.call::<{}>(\"{}\", &rpc_params).await",
                response_type, rpc.name
            )
            .expect("Failed to write method body");
        } else {
            // For methods with no parameters, use empty array
            writeln!(
                buf,
                "        self.call::<{}>(\"{}\", &[]).await",
                response_type, rpc.name
            )
            .expect("Failed to write method body");
        }
        writeln!(buf, "    }}").expect("Failed to write method closing brace");

        buf
    }

    /// Get response type for a method
    fn get_response_type(&self, rpc: &RpcDef) -> String {
        if rpc.result.is_none() {
            "()".to_string()
        } else {
            format!(
                "{}Response",
                snake_to_pascal_case(
                    &protocol_rpc_method_to_rust_name(self.protocol.as_str(), &rpc.name,)
                        .unwrap_or_else(|e| panic!("{}", e)),
                )
            )
        }
    }
}
