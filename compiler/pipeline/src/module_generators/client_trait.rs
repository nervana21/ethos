//! Client trait module generator
//!
//! Generates the client trait that defines the interface for RPC clients.

use std::path::PathBuf;

use super::ModuleGenerator;
use crate::generation_context::GenerationContext;
use crate::PipelineError;

/// Generator for the client trait module
pub struct ClientTraitModuleGenerator;

impl ModuleGenerator for ClientTraitModuleGenerator {
    fn module_name(&self) -> &str { "client_trait" }

    fn generate_files(
        &self,
        ctx: &GenerationContext,
    ) -> Result<Vec<(String, String)>, PipelineError> {
        ctx.versioned_registry
            .generate_client_trait(&ctx.rpc_methods)
            .map_err(|e| PipelineError::Message(e.to_string()))
    }

    fn output_subdir(&self, _ctx: &GenerationContext) -> PathBuf { PathBuf::from("client_trait") }
}
