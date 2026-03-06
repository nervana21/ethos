//! Version-specific client trait generator
//!
//! This module enhances the client trait generator to use version-specific
//! type metadata for generating accurate method signatures and parameter types.

use std::fmt::Write as _;

use ir::RpcDef;
use types::type_adapter::TypeAdapter;
use types::{Implementation, ProtocolVersion, TypeRegistry};

use super::doc_comment::format_doc_comment;
use super::fee_rate_utils::{methods_use_amounts_map, methods_use_get_block_template_request};
use crate::generators::version_specific_response_type::record_external_symbol_usage;
use crate::utils::{
    canonical_from_adapter_method, protocol_rpc_method_to_rust_name, sanitize_external_identifier,
    snake_to_pascal_case,
};
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
            "bitcoin_core" => {
                include_str!("../../templates/bitcoin_core/client_trait.rs")
            }
            _ => panic!("Unsupported protocol: {}", self.protocol),
        };
        let client_trait = self.render_client_trait(template, &available_methods);

        // render mod.rs that re-exports the trait
        let client_name = match self.protocol.as_str() {
            "bitcoin_core" => "BitcoinClient",
            _ => panic!("Unsupported protocol: {}", self.protocol),
        };
        let exported_trait_name = client_name.to_string();
        let protocol_display = self.protocol.display_name();
        let version_short = self.version.short();
        let mod_rs = format!(
            "//! Auto-generated module for {client_name}\n\
             //!\n\
             //! Generated for {protocol_display} {version_short}\n\
             //!\n\
             //! This module contains version-specific method signatures that may\n\
             //! not be compatible with other {protocol_display} versions.\n\
             pub mod client;\n\
             pub use self::client::{exported_trait_name};\n"
        );

        vec![("client.rs".into(), client_trait), ("mod.rs".into(), mod_rs)]
    }
}

impl VersionSpecificClientTraitGenerator {
    // Removed filter_methods_for_version as it's no longer needed with RpcDef

    /// Render the client trait with version-specific information
    fn render_client_trait(&self, template: &str, methods: &[&RpcDef]) -> String {
        let mut out = template.to_owned();

        let version_str = self.version.short();
        out = out.replace("{{VERSION}}", &version_str);

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

        let adapter = self.get_adapter();
        let uses_fee_rate = crate::generators::fee_rate_utils::methods_use_fee_rate(
            methods.iter().map(|m| *m),
            adapter.as_ref(),
        );
        let uses_sendall_recipient =
            crate::generators::fee_rate_utils::methods_use_sendall_recipient(
                methods.iter().map(|m| *m),
                adapter.as_ref(),
            );
        let uses_get_block_template_request =
            methods_use_get_block_template_request(methods.iter().map(|m| *m), adapter.as_ref());
        let uses_amounts_map =
            methods_use_amounts_map(methods.iter().map(|m| *m), adapter.as_ref());

        // Add necessary imports
        if uses_sendall_recipient || uses_get_block_template_request {
            let params_mod = self.protocol.client_dir_name();
            let mut params_types = Vec::new();
            if uses_sendall_recipient {
                params_types.push("SendallRecipient");
            }
            if uses_get_block_template_request {
                params_types.push("GetBlockTemplateRequest");
            }
            imports.push(format!(
                "use crate::{}::params::{{{}}}",
                params_mod,
                params_types.join(", ")
            ));
        }
        if uses_hash_or_height {
            imports.push("use crate::types::HashOrHeight".to_string());
        }
        if uses_public_key {
            // Prefer the canonical bitcoin crate path to avoid duplicate type re-exports
            imports.push("use bitcoin::PublicKey".to_string());
            // Record external symbol usage so lib.rs can re-export it
            record_external_symbol_usage("bitcoin", "PublicKey");
        }
        if uses_fee_rate {
            imports.push("use crate::types::FeeRate".to_string());
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
        if rpc.requires_private_keys {
            writeln!(buf, "    ///").expect("write");
            writeln!(buf, "    /// Requires wallet private keys to be available (e.g. unlocked).")
                .expect("write");
        }

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

        // Build arguments once so they're in scope for both params_sig and method body
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

        // Generate individual parameters instead of struct
        let params_sig = if rpc.params.is_empty() {
            "".to_string()
        } else {
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
        if rpc.requires_private_keys {
            writeln!(buf, "    ///").expect("write");
            writeln!(buf, "    /// Requires wallet private keys to be available (e.g. unlocked).")
                .expect("write");
        }

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
            let adapter = self.get_adapter();
            // Create params array from individual parameters
            // Optional parameters (Option<T>) should only be included if they're Some(...)
            // Use rpc_params as variable name to avoid conflict with parameter named "params"
            writeln!(buf, "        let mut rpc_params = vec![];")
                .expect("Failed to write params array initialization");
            for (param, arg) in rpc.params.iter().zip(arguments.iter()) {
                let param_name = sanitize_external_identifier(&param.name);
                let (base_ty, _) =
                    TypeRegistry::map_argument_type_with_adapter(arg, adapter.as_ref());
                let field_name = param.name.as_str();
                let is_fee_rate = field_name == "fee_rate";
                let is_max_fee_rate = field_name == "maxfeerate";
                let is_amounts_map = field_name == "amounts"
                    && base_ty.contains("HashMap")
                    && base_ty.contains("Amount");

                // Serialize parameters according to their semantic type and JSON unit.
                // - FeeRate: fee_rate → sat/vB numeric, maxfeerate → BTC/kvB numeric
                // - bitcoin::Amount: BTC floats via to_btc()
                // - sendmany "amounts": HashMap<Address, Amount> → JSON object with BTC float values
                // - everything else: default json!(param)
                let push_expr = if base_ty == "FeeRate" && is_fee_rate {
                    format!("serde_json::json!({}.to_sat_per_vb_floor())", param_name)
                } else if base_ty == "FeeRate" && is_max_fee_rate {
                    format!(
                        "serde_json::json!(({}.to_sat_per_kvb_floor() as f64) / 100_000_000.0)",
                        param_name
                    )
                } else if base_ty == "bitcoin::Amount" {
                    format!("serde_json::json!({}.to_btc())", param_name)
                } else if is_amounts_map {
                    format!(
                        "serde_json::json!({}.iter().map(|(k, v)| (serde_json::to_value(k).unwrap().as_str().unwrap().to_string(), v.to_btc())).collect::<std::collections::HashMap<_, _>>())",
                        param_name
                    )
                } else {
                    format!("serde_json::json!({})", param_name)
                };

                if !param.required {
                    let val_expr = if base_ty == "FeeRate" && is_fee_rate {
                        "serde_json::json!(val.to_sat_per_vb_floor())"
                    } else if base_ty == "FeeRate" && is_max_fee_rate {
                        "serde_json::json!((val.to_sat_per_kvb_floor() as f64) / 100_000_000.0)"
                    } else if base_ty == "bitcoin::Amount" {
                        "serde_json::json!(val.to_btc())"
                    } else if is_amounts_map {
                        "serde_json::json!(val.iter().map(|(k, v)| (serde_json::to_value(k).unwrap().as_str().unwrap().to_string(), v.to_btc())).collect::<std::collections::HashMap<_, _>>())"
                    } else {
                        "serde_json::json!(val)"
                    };
                    writeln!(buf, "        if let Some(val) = {} {{", param_name)
                        .expect("Failed to write optional parameter check");
                    writeln!(buf, "            rpc_params.push({});", val_expr)
                        .expect("Failed to write optional parameter push");
                    writeln!(buf, "        }}")
                        .expect("Failed to write optional parameter closing");
                } else {
                    writeln!(buf, "        rpc_params.push({});", push_expr)
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
            writeln!(buf, "        self.call::<{}>(\"{}\", &[]).await", response_type, rpc.name)
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
            match canonical_from_adapter_method(self.protocol.as_str(), &rpc.name) {
                Ok(canonical) => format!("{}Response", canonical),
                Err(_) => format!(
                    "{}Response",
                    snake_to_pascal_case(
                        &protocol_rpc_method_to_rust_name(self.protocol.as_str(), &rpc.name)
                            .unwrap_or_else(|e| panic!("{}", e)),
                    )
                ),
            }
        }
    }
}
