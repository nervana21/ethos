//! Utilities for generating RPC method signatures and implementations from RPC definitions.

use ir::RpcDef;

use super::doc_comment::format_doc_comment;

/// Tiny DSL to turn one RpcDef into its doc-comment + fn
pub struct MethodTemplate<'a> {
    method: &'a RpcDef,
    type_adapter: &'a dyn types::type_adapter::TypeAdapter,
}

impl<'a> MethodTemplate<'a> {
    /// Create a new MethodTemplate for the given RpcDef
    pub fn new(method: &'a RpcDef, type_adapter: &'a dyn types::type_adapter::TypeAdapter) -> Self {
        MethodTemplate { method, type_adapter }
    }

    /// Generate parameter struct for methods that require argument reordering
    pub fn generate_param_struct(&self) -> Option<String> {
        use crate::utils::{needs_parameter_reordering, reorder_arguments_for_rust_signature};

        // Convert RpcDef params to the format expected by the utility functions
        let arguments: Vec<types::Argument> = self
            .method
            .params
            .iter()
            .map(|param| {
                let protocol_type = param.param_type.protocol_type.as_ref().unwrap_or_else(|| {
                    panic!(
						"Parameter '{}' in method '{}' is missing protocol_type. Rust type name is '{}'. \
						All parameters must have protocol_type set for proper type categorization.",
						param.name, self.method.name, param.param_type.name
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

        if !needs_parameter_reordering(&arguments) {
            return None;
        }

        let (reordered_args, param_mapping) = reorder_arguments_for_rust_signature(&arguments);
        let struct_name = format!(
            "{}Params",
            crate::utils::snake_to_pascal_case(&crate::utils::rpc_method_to_rust_name(
                &self.method.name
            ))
        );

        let mut fields = Vec::new();
        for arg in &reordered_args {
            let field_name =
                format!("_{}", crate::utils::sanitize_external_identifier(&arg.names[0]));

            // Use protocol-specific type adapter to map parameter types
            let (base_ty, _) =
                types::TypeRegistry::map_argument_type_with_adapter(arg, self.type_adapter);
            let field_type =
                if !arg.required { format!("Option<{}>", base_ty) } else { base_ty.to_string() };

            fields.push(format!("    pub {}: {},", field_name, field_type));
        }

        // Generate custom serialization that converts struct to array in original order
        let mut serialize_fields = Vec::new();
        for (original_idx, _) in arguments.iter().enumerate() {
            let reordered_idx = param_mapping
                .iter()
                .position(|&x| x == original_idx)
                .expect("Parameter mapping should contain all original indices");
            let arg = &reordered_args[reordered_idx];
            let field_name =
                &format!("_{}", crate::utils::sanitize_external_identifier(&arg.names[0]));
            serialize_fields.push(format!("        seq.serialize_element(&self.{})?;", field_name));
        }

        // Generate documentation for the struct
        let struct_doc = if !self.method.description.trim().is_empty() {
            let sanitized_desc = format_doc_comment(&self.method.description);
            format!("/// Parameters for the `{}` RPC method.\n{}", self.method.name, sanitized_desc)
        } else {
            format!("/// Parameters for the `{}` RPC method.", self.method.name)
        };

        // Generate field documentation
        let mut documented_fields = Vec::new();
        for arg in reordered_args.iter() {
            let field_name =
                format!("_{}", crate::utils::sanitize_external_identifier(&arg.names[0]));

            // Use protocol-specific type adapter to map parameter types
            let (base_ty, _) =
                types::TypeRegistry::map_argument_type_with_adapter(arg, self.type_adapter);
            let field_type =
                if !arg.required { format!("Option<{}>", base_ty) } else { base_ty.to_string() };

            let field_doc = if !arg.description.trim().is_empty() {
                let sanitized_field_desc = format_doc_comment(&arg.description);
                if !sanitized_field_desc.is_empty() {
                    // Add proper indentation to each line
                    sanitized_field_desc
                        .lines()
                        .map(|line| format!("    {}", line))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    format!("    /// {}", arg.names[0])
                }
            } else {
                format!("    /// {}", arg.names[0])
            };

            documented_fields
                .push(format!("{}\n    pub {}: {},", field_doc, field_name, field_type));
        }

        Some(format!(
            "{}\n\
            #[derive(Debug, Clone, Deserialize)]\n\
            pub struct {} {{\n\
            {}\n\
            }}\n\
            \n\
            impl serde::Serialize for {} {{\n\
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>\n\
                where\n\
                S: serde::Serializer,\n\
                {{\n\
                    let mut seq = serializer.serialize_seq(Some({}))?;\n\
            {}\n\
                    seq.end()\n\
                }}\n\
            }}",
            struct_doc,
            struct_name,
            documented_fields.join("\n"),
            struct_name,
            arguments.len(),
            serialize_fields.join("\n")
        ))
    }

    /// Render the /// doc lines
    fn doc(&self) -> String {
        let mut lines = Vec::new();
        let mut seen_content = false;

        for line in self.method.description.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if seen_content {
                    lines.push("    ///".to_string());
                }
            } else {
                seen_content = true;
                lines.push(format!("    /// {}", trimmed));
            }
        }

        while matches!(lines.last(), Some(l) if l.trim().is_empty()) {
            lines.pop();
        }

        lines.join("\n")
    }

    /// Build the `, name: Type, ...` part of the fn signature
    fn signature(&self) -> String {
        use crate::utils::needs_parameter_reordering;

        // Convert RpcDef params to the format expected by the utility functions
        let arguments: Vec<types::Argument> = self
            .method
            .params
            .iter()
            .map(|param| {
                // Require protocol_type for proper type categorization - must match IR data quality
                let protocol_type = param.param_type.protocol_type.as_ref().unwrap_or_else(|| {
                    panic!(
						"Parameter '{}' in method '{}' is missing protocol_type. Rust type name is '{}'. \
							All parameters must have protocol_type set for proper type categorization.",
						param.name, self.method.name, param.param_type.name
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

        // Check if this method requires argument reordering
        if needs_parameter_reordering(&arguments) {
            // Use a parameter struct for methods with ordering issues
            let struct_name = format!(
                "{}Params",
                crate::utils::snake_to_pascal_case(&crate::utils::rpc_method_to_rust_name(
                    &self.method.name
                ))
            );
            format!(", params: {}", struct_name)
        } else {
            // Use individual parameters for methods that don't require argument reordering
            let args = arguments
                .iter()
                .map(|arg| {
                    // Add underscore prefix to all parameter names for consistency and clarity.
                    // This distinguishes parameters from other identifiers and follows Rust conventions
                    // for intentionally prefixed names. The special case for "type" uses r#_type
                    // to properly escape the reserved keyword.
                    let name =
                        format!("_{}", crate::utils::sanitize_external_identifier(&arg.names[0]));

                    // Use protocol-specific type adapter to map parameter types
                    let (base_ty, _) =
                        types::TypeRegistry::map_argument_type_with_adapter(arg, self.type_adapter);
                    let ty = if !arg.required {
                        format!("Option<{}>", base_ty)
                    } else {
                        base_ty.clone()
                    };
                    format!("{name}: {ty}")
                })
                .collect::<Vec<_>>();
            if args.is_empty() {
                "".into()
            } else {
                format!(", {}", args.join(", "))
            }
        }
    }

    /// Decide whether we return `()` or `FooResponse`
    fn return_type(&self) -> String {
        // Always return the Response type for consistency
        // The response generator will create appropriate types for all methods
        format!(
            "{}Response",
            crate::utils::snake_to_pascal_case(&crate::utils::rpc_method_to_rust_name(
                &self.method.name
            ))
        )
    }

    /// Build the lines for parameter serialization
    /// Returns code that builds the params vector, handling optional parameters correctly
    pub fn json_params(&self) -> String {
        use crate::utils::needs_parameter_reordering;

        // Convert RpcDef params to the format expected by the utility functions
        let arguments: Vec<types::Argument> = self
            .method
            .params
            .iter()
            .map(|param| types::Argument {
                names: vec![param.name.clone()],
                type_: param.param_type.name.clone(),
                required: param.required,
                description: param.description.clone(),
                oneline_description: String::new(),
                also_positional: false,
                hidden: false,
                type_str: None,
            })
            .collect();

        if needs_parameter_reordering(&arguments) {
            // For methods that require argument reordering, serialize from the parameter struct
            // The custom Serialize impl already serializes the struct as an array, so we need to spread it
            "            ..serde_json::to_value(&params).unwrap().as_array().unwrap().clone()"
                .to_string()
        } else {
            // For methods not needing reordering, serialize individual parameters
            // Check if there are any optional parameters
            let has_optional = arguments.iter().any(|arg| !arg.required);

            if has_optional {
                // If there are optional parameters, use conditional logic
                let mut lines = Vec::new();
                for arg in &arguments {
                    let name =
                        &format!("_{}", crate::utils::sanitize_external_identifier(&arg.names[0]));
                    if arg.required {
                        // Required parameter: always include
                        lines.push(format!(
                            "            rpc_params.push(serde_json::json!({name}));"
                        ));
                    } else {
                        // Optional parameter: only include if Some
                        lines.push(format!("            if let Some(val) = {name} {{"));
                        lines.push(format!(
                            "                rpc_params.push(serde_json::json!(val));"
                        ));
                        lines.push(format!("            }}"));
                    }
                }
                lines.join("\n")
            } else {
                // All parameters are required, use vec![] syntax
                arguments
                    .iter()
                    .map(|arg| {
                        let name = &format!(
                            "_{}",
                            crate::utils::sanitize_external_identifier(&arg.names[0])
                        );
                        format!("            serde_json::json!({name}),")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
    }

    /// Assemble the full async fn stub
    fn body(&self) -> String {
        let name = crate::utils::rpc_method_to_rust_name(&self.method.name);
        let sig = self.signature();
        let ret = self.return_type();
        let json = self.json_params();
        let rpc = &self.method.name;

        // Add clippy allow for too many arguments if needed
        let clippy_allow = if self.method.params.len() > 7 {
            "#[allow(clippy::too_many_arguments)]\n    "
        } else {
            ""
        };

        // Check if we need to use mut params (for optional parameter handling)
        // This happens when there are optional parameters and no parameter reordering
        use crate::utils::needs_parameter_reordering;
        let arguments: Vec<types::Argument> = self
            .method
            .params
            .iter()
            .map(|param| types::Argument {
                names: vec![param.name.clone()],
                type_: param.param_type.name.clone(),
                required: param.required,
                description: param.description.clone(),
                oneline_description: String::new(),
                also_positional: false,
                hidden: false,
                type_str: None,
            })
            .collect();

        let needs_mut = !needs_parameter_reordering(&arguments)
            && self.method.params.iter().any(|p| !p.required);
        let params_decl =
            if needs_mut { "let mut rpc_params = vec![];" } else { "let rpc_params = vec![" };
        let params_close = if needs_mut { "" } else { "];" };

        format!(
            "{clippy_allow}async fn {name}(&self{sig}) -> Result<{ret}, TransportError> {{
        {params_decl}
{json}
        {params_close}
        self.dispatch_json::<{ret}>(\"{rpc}\", &rpc_params).await
    }}"
        )
    }

    /// Render the method as a string
    pub fn render(&self) -> String { format!("{}\n{}", self.doc(), self.body()) }
}
