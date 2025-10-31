//! Test node module generator
//!
//! Generates test node modules for integration testing.

use std::fs;
use std::path::PathBuf;

use codegen::generators::test_node::TestNodeGenerator;
use codegen::utils::generate_mod_rs;
use codegen::CodeGenerator;

use super::ModuleGenerator;
use crate::generation_context::GenerationContext;
use crate::PipelineError;

/// Generator for the test node module
pub struct TestNodeModuleGenerator;

impl ModuleGenerator for TestNodeModuleGenerator {
    fn module_name(&self) -> &str { "test_node" }

    fn generate_files(
        &self,
        ctx: &GenerationContext,
    ) -> Result<Vec<(String, String)>, PipelineError> {
        // Set the adapter context for method name conversion

        let tn_files =
            TestNodeGenerator::new(ctx.versioned_registry.version().clone(), ctx.implementation)
                .generate(&ctx.rpc_methods);
        Ok(tn_files)
    }

    fn output_subdir(&self, ctx: &GenerationContext) -> PathBuf {
        // Write test node files to implementation-specific clients submodule within the main library
        let clients_dir_name = ctx.implementation.client_dir_name();
        PathBuf::from(clients_dir_name)
    }

    fn generate_and_write(&self, ctx: &GenerationContext) -> Result<(), PipelineError> {
        let files = self.generate_files(ctx)?;
        let clients_dir = ctx.base_output_dir.join(self.output_subdir(ctx));
        fs::create_dir_all(&clients_dir)?;

        // Write the generated files
        codegen::write_generated(&clients_dir, &files)?;

        // Generate mod.rs with the clients directory name
        let clients_dir_name = ctx.implementation.client_dir_name();
        generate_mod_rs(&clients_dir, clients_dir_name)?;

        Ok(())
    }
}
