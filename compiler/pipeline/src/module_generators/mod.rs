//! Module generators for the code generation pipeline.
//!
//! This module provides standardized generators that implement the ModuleGenerator
//! trait to generate different parts of the output codebase.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::generation_context::GenerationContext;
use crate::PipelineError;

/// Trait for generating code modules
pub trait ModuleGenerator {
    /// Get the name of this module
    fn module_name(&self) -> &str;

    /// Generate the files for this module
    fn generate_files(
        &self,
        ctx: &GenerationContext,
    ) -> Result<Vec<(String, String)>, PipelineError>;

    /// Get the output subdirectory for this module
    fn output_subdir(&self, ctx: &GenerationContext) -> PathBuf;

    /// Whether this module should generate a mod.rs file
    fn should_generate_mod_rs(&self) -> bool { true }

    /// Generate and write the module files
    fn generate_and_write(&self, ctx: &GenerationContext) -> Result<(), PipelineError> {
        let files = self.generate_files(ctx)?;
        let output_dir = ctx.base_output_dir.join(self.output_subdir(ctx));

        // Write the generated files
        codegen::write_generated(&output_dir, &files)?;

        // Generate mod.rs if needed
        if self.should_generate_mod_rs() {
            let mod_rs = output_dir.join("mod.rs");
            let mut content = String::new();

            for (name, _) in &files {
                let module_name = name.strip_suffix(".rs").unwrap_or(name);
                if module_name != "mod" {
                    writeln!(content, "pub mod {};", module_name)?;
                    writeln!(content, "pub use {}::*;", module_name)?;
                }
            }

            std::fs::write(&mod_rs, content)?;
        }

        Ok(())
    }
}

pub mod client_trait;
pub mod lib_rs;
pub mod node_manager;
pub mod response_types;
pub mod test_node;
pub mod transport;
