//! Transport module generator
//!
//! Generates the transport layer including method wrappers and infrastructure.

use std::fmt::Write as _;
use std::path::PathBuf;

use adapters::bitcoin_core::openrpc_schema_validate::WireSchemaRegistry;
use codegen::{
    write_generated, CodeGenerator, MethodWrapperGenerator, TransportInfrastructureGenerator,
};

use super::ModuleGenerator;
use crate::generation_context::GenerationContext;
use crate::PipelineError;

/// Generator for the transport module
pub struct TransportModuleGenerator;

impl ModuleGenerator for TransportModuleGenerator {
    fn module_name(&self) -> &str { "transport" }

    fn generate_files(
        &self,
        ctx: &GenerationContext,
    ) -> Result<Vec<(String, String)>, PipelineError> {
        // Generate method wrapper files
        let tx_files = MethodWrapperGenerator::new(ctx.implementation.as_str().to_string())
            .generate(&ctx.rpc_methods);

        // Generate transport infrastructure files
        let core_files = TransportInfrastructureGenerator::new(ctx.transport_protocol())
            .generate(&ctx.rpc_methods);

        // Generate RPC client from template
        let rpc_client = self.generate_rpc_client(&ctx.transport_protocol())?;

        // Wire schema registry for optional `schema-validate` feature.
        let wire_schema = match &ctx.openrpc_document {
            Some(doc) => {
                let registry = WireSchemaRegistry::from_openrpc_document(doc)
                    .map_err(PipelineError::Message)?;
                registry.emit_generated_module_source()
            }
            None => STUB_WIRE_SCHEMA.to_owned(),
        };

        // Combine all transport files
        let mut all_files = tx_files;
        all_files.extend(core_files);
        all_files.push(("rpc_client.rs".to_string(), rpc_client));
        all_files.push(("wire_schema.rs".to_string(), wire_schema));

        Ok(all_files)
    }

    fn output_subdir(&self, _ctx: &GenerationContext) -> PathBuf { PathBuf::from("transport") }

    fn generate_and_write(&self, ctx: &GenerationContext) -> Result<(), PipelineError> {
        let files = self.generate_files(ctx)?;
        let output_dir = ctx.base_output_dir.join(self.output_subdir(ctx));

        // Separate method files from infrastructure files
        let (method_files, infrastructure_files): (Vec<_>, Vec<_>) =
            files.iter().partition(|(name, _)| {
                // Method files are categorized files (blockchain.rs, wallet.rs, etc.)
                // Infrastructure files are core.rs, rpc_client.rs, wire_schema.rs, etc.
                !name.contains("core")
                    && !name.contains("rpc_client")
                    && !name.contains("wire_schema")
            });

        // Convert references to owned values
        let method_files: Vec<_> = method_files.into_iter().cloned().collect();
        let infrastructure_files: Vec<_> = infrastructure_files.into_iter().cloned().collect();

        // Write infrastructure files to transport root
        write_generated(&output_dir, &infrastructure_files)?;

        // Create methods subdirectory and write method files there
        let methods_dir = output_dir.join("methods");
        std::fs::create_dir_all(&methods_dir)?;
        write_generated(&methods_dir, &method_files)?;

        // Generate mod.rs for transport root with custom content
        let mod_rs = output_dir.join("mod.rs");
        let mut content = String::new();
        writeln!(content, "pub mod core;")?;
        writeln!(content, "pub use core::{{TransportTrait, DefaultTransport, TransportError}};")?;
        writeln!(content, "pub mod rpc_client;")?;
        writeln!(content, "pub use rpc_client::RpcClient;")?;
        writeln!(content, "#[cfg(feature = \"schema-validate\")]")?;
        writeln!(content, "pub mod wire_schema;")?;
        writeln!(content, "pub mod methods;")?;
        std::fs::write(&mod_rs, content)?;

        // Generate mod.rs for methods subdirectory
        let methods_mod_rs = methods_dir.join("mod.rs");
        let mut methods_content = String::new();
        for (name, _) in &method_files {
            let module_name = name.strip_suffix(".rs").unwrap_or(name);
            if module_name != "mod" {
                writeln!(methods_content, "pub mod {};", module_name)?;
                writeln!(methods_content, "pub use {}::*;", module_name)?;
            }
        }
        std::fs::write(&methods_mod_rs, methods_content)?;

        Ok(())
    }
}

impl TransportModuleGenerator {
    /// Generate RPC client from template
    fn generate_rpc_client(&self, transport_protocol: &str) -> Result<String, PipelineError> {
        // Load the template
        let template = include_str!("../../templates/rpc_client.rs");

        // Generate protocol-specific transport constructor call
        let transport_constructor = match transport_protocol {
            "http" => "DefaultTransport::new(url, None)",
            "unix" => "DefaultTransport::new(url)",
            _ => panic!(
                "Unsupported transport protocol: {}. Supported protocols: http, unix",
                transport_protocol
            ),
        };

        // Process the template
        Ok(template.replace("{{TRANSPORT_CONSTRUCTOR}}", transport_constructor))
    }
}

const STUB_WIRE_SCHEMA: &str = r#"//! Stub OpenRPC wire schema registry (no OpenRPC dump supplied at codegen).
//!
//! Enabled with feature `schema-validate`. Validation is a no-op until codegen is re-run
//! with an OpenRPC document (`--openrpc` / default `resources/ir/openrpc.json`).

#![cfg(feature = "schema-validate")]

use serde_json::Value;

/// No-op: OpenRPC schemas were not embedded at codegen time.
pub fn validate_params(_method: &str, _params: &[Value]) -> Result<(), String> {
    Ok(())
}

/// No-op: OpenRPC schemas were not embedded at codegen time.
pub fn validate_result(_method: &str, _result: &Value) -> Result<(), String> {
    Ok(())
}
"#;
