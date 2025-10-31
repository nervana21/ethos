//! Transport module generator
//!
//! Generates the transport layer including method wrappers and infrastructure.

use std::fmt::Write as _;
use std::path::PathBuf;

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
        let tx_files = MethodWrapperGenerator::new(
            ctx.client_name(),
            ctx.full_crate_name(),
            ctx.implementation.as_str().to_string(),
        )
        .generate(&ctx.rpc_methods);

        // Generate transport infrastructure files
        let core_files = TransportInfrastructureGenerator::new(ctx.transport_protocol())
            .generate(&ctx.rpc_methods);

        // Generate RPC client from template
        let rpc_client = self.generate_rpc_client(&ctx.transport_protocol())?;

        // Combine all transport files
        let mut all_files = tx_files;
        all_files.extend(core_files);
        all_files.push(("rpc_client.rs".to_string(), rpc_client));

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
                // Infrastructure files are core.rs, rpc_client.rs, etc.
                !name.contains("core") && !name.contains("rpc_client")
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
        writeln!(content, "pub mod methods;")?;
        writeln!(content, "pub use ::transport::get_random_free_port;")?;
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
            "http" => "DefaultTransport::new(socket_path, None)",
            "unix" => "DefaultTransport::new(socket_path)",
            _ => panic!(
                "Unsupported transport protocol: {}. Supported protocols: http, unix",
                transport_protocol
            ),
        };

        // Process the template
        Ok(template.replace("{{TRANSPORT_CONSTRUCTOR}}", transport_constructor))
    }
}
