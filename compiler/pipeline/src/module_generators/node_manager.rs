//! Node manager module generator
//!
//! Generates the node manager module for managing Bitcoin protocol nodes in test environments.

use std::path::PathBuf;

use codegen::{CodeGenerator, NodeManagerGenerator};

use super::ModuleGenerator;
use crate::generation_context::GenerationContext;
use crate::PipelineError;

/// Generator for the node manager module
pub struct NodeManagerModuleGenerator;

impl ModuleGenerator for NodeManagerModuleGenerator {
    fn module_name(&self) -> &str { "node" }

    fn generate_files(
        &self,
        ctx: &GenerationContext,
    ) -> Result<Vec<(String, String)>, PipelineError> {
        let node_manager_files =
            NodeManagerGenerator::new(ctx.implementation).generate(&ctx.rpc_methods);
        Ok(node_manager_files)
    }

    fn output_subdir(&self, _ctx: &GenerationContext) -> PathBuf { PathBuf::from("node") }
}
