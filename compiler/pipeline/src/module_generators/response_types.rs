//! Response types module generator
//!
//! Generates strongly-typed response structs for RPC methods.

use std::fmt::Write as _;
use std::path::PathBuf;

use codegen::write_generated;

use super::ModuleGenerator;
use crate::generation_context::GenerationContext;
use crate::PipelineError;

/// Generator for the response types module
pub struct ResponseTypesModuleGenerator;

impl ModuleGenerator for ResponseTypesModuleGenerator {
    fn module_name(&self) -> &str { "responses" }

    fn generate_files(
        &self,
        ctx: &GenerationContext,
    ) -> Result<Vec<(String, String)>, PipelineError> {
        ctx.versioned_registry
            .generate_response_types(&ctx.rpc_methods)
            .map_err(|e| PipelineError::Message(e.to_string()))
    }

    fn output_subdir(&self, _ctx: &GenerationContext) -> PathBuf { PathBuf::from("types") }

    fn generate_and_write(&self, ctx: &GenerationContext) -> Result<(), PipelineError> {
        let files = self.generate_files(ctx)?;
        let output_dir = ctx.base_output_dir.join(self.output_subdir(ctx));

        // Generate version-specific module name using the dedicated method
        let module_name = ctx.versioned_registry.version().as_version_module_name();
        let version_dir = output_dir.join(&module_name);
        std::fs::create_dir_all(&version_dir)?;

        // Write categorized response files to version-specific subdirectory
        write_generated(&version_dir, &files)?;

        // Ensure no stale common.rs remains from older generations
        let common_file = output_dir.join("common.rs");
        if common_file.exists() {
            std::fs::remove_file(&common_file)?;
        }

        // Generate mod.rs for types root with only the version-specific module
        let types_mod_rs = output_dir.join("mod.rs");
        let mut types_content = String::new();
        writeln!(types_content, "pub mod {};", module_name)?;
        writeln!(types_content, "pub use {}::*;", module_name)?;
        // Common type aliases and re-exports used in generated code
        writeln!(types_content, "pub use bitcoin::PublicKey;")?;
        writeln!(types_content, "#[derive(Debug, serde::Serialize)]")?;
        writeln!(
            types_content,
            "pub enum HashOrHeight {{ Hash(bitcoin::BlockHash), Height(u32) }}"
        )?;
        writeln!(types_content, "pub type ShortChannelId = String;")?;
        std::fs::write(&types_mod_rs, types_content)?;

        // Generate mod.rs for version-specific subdirectory
        let version_mod_rs = version_dir.join("mod.rs");
        let mut version_content = String::new();
        for (name, _) in &files {
            let module_name = name.strip_suffix(".rs").unwrap_or(name);
            if module_name != "mod" {
                writeln!(version_content, "pub mod {};", module_name)?;
                writeln!(version_content, "pub use {}::*;", module_name)?;
            }
        }
        std::fs::write(&version_mod_rs, version_content)?;

        Ok(())
    }
}
