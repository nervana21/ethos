//! Code generation orchestration for the pipeline.
//!
//! This module handles high-level code generation coordination including semantic analysis
//! and the main code generation process.

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use analysis::{CompilerContext, CompilerPhase, SemanticAnalyzer};
use codegen::generators::version_specific_response_type::set_external_symbol_recorder;
use codegen::generators::versioned_registry::VersionedGeneratorRegistry;
use ir::ProtocolIR;
use types::{Implementation, ProtocolVersion};

use crate::generation_context::{GenerationContext, UsedExternalSymbols};
use crate::module_generators::client_trait::ClientTraitModuleGenerator;
use crate::module_generators::lib_rs::LibRsModuleGenerator;
use crate::module_generators::node_manager::NodeManagerModuleGenerator;
use crate::module_generators::response_types::ResponseTypesModuleGenerator;
use crate::module_generators::test_node::TestNodeModuleGenerator;
use crate::module_generators::transport::TransportModuleGenerator;
use crate::module_generators::ModuleGenerator;
use crate::template_management::copy_templates_to;
use crate::{feature_aware_cargo, PipelineError};

// Bridge from generators back into the pipeline's shared collector (safe, no unsafe)
static EXTERNAL_COLLECTOR: OnceLock<Mutex<Option<Arc<UsedExternalSymbols>>>> = OnceLock::new();

fn pipeline_external_symbol_recorder(crate_name: &str, symbol: &str) {
    if let Some(slot) = EXTERNAL_COLLECTOR.get() {
        if let Some(ref collector) = *slot.lock().expect("collector mutex poisoned") {
            collector.record(crate_name, symbol);
        }
    }
}

/// Analyze the implementation against the protocol IR using semantic analysis
///
/// This function creates a compiler context from the ProtocolIR and runs
/// semantic analysis to validate semantic invariants.
///
/// # Arguments
///
/// * `implementation` - Name of the implementation
/// * `protocol_ir` - The ProtocolIR to analyze
/// * `version` - Implementation version (e.g., v30.0.0 for Bitcoin Core)
/// * `ir_source_path` - Path to the input IR file (for reference)
///
/// # Returns
///
/// Returns `Result<CompilerContext>` containing the analyzed context
pub fn analyze_implementation(
    implementation: Implementation,
    protocol_ir: ProtocolIR,
    version: &ProtocolVersion,
    ir_source_path: std::path::PathBuf,
) -> Result<CompilerContext, PipelineError> {
    let mut ctx = CompilerContext::new(implementation, version.clone(), Some(ir_source_path), None);

    ctx.update_ir(protocol_ir);

    let semantic_analyzer = SemanticAnalyzer::new();
    semantic_analyzer.run(&mut ctx).map_err(|e| PipelineError::Message(e.to_string()))?;

    Ok(ctx)
}

/// Generate all the code into the specified output directory
///
/// # Arguments
///
/// * `out_dir` - The output directory to write generated code to
/// * `compiler_ctx` - The compiler context containing the blueprint with semantic analysis results
pub fn generate_into(out_dir: &Path, compiler_ctx: &CompilerContext) -> Result<(), PipelineError> {
    // Set up directory structure
    fs::create_dir_all(out_dir)?;

    // Extract data from CompilerContext
    let implementation = compiler_ctx.implementation;
    let version = compiler_ctx.version.clone();

    // Extract RPC methods from ProtocolIR
    let rpc_methods: Vec<_> = compiler_ctx.ir.get_rpc_methods().into_iter().cloned().collect();

    copy_templates_to(out_dir, implementation)?;

    // Initialize version-specific generator registry from IR
    let versioned_registry = VersionedGeneratorRegistry::from_ir(
        implementation.as_str(),
        version.clone(),
        &compiler_ctx.ir,
    )
    .map_err(|e| {
        PipelineError::Message(format!(
            "Failed to initialize versioned generators for {}: {}.",
            implementation.as_str(),
            e
        ))
    })?;

    let ctx = GenerationContext::builder()
        .implementation(implementation)
        .versioned_registry(versioned_registry)
        .rpc_methods(rpc_methods)
        .protocol_ir(compiler_ctx.ir.clone())
        .used_external_symbols(Arc::new(UsedExternalSymbols::new()))
        .diagnostics(compiler_ctx.diagnostics.clone())
        .output_dir(out_dir.to_path_buf())
        .build()?;

    // Initialize the global recorder hook with the context's collector
    let slot = EXTERNAL_COLLECTOR.get_or_init(|| Mutex::new(None));
    *slot.lock().expect("collector mutex poisoned") = Some(ctx.used_external_symbols.clone());
    // Test configs depend on bitcoin::Network; record it up-front so lib.rs re-exports it
    ctx.used_external_symbols.record("bitcoin", "Network");
    set_external_symbol_recorder(pipeline_external_symbol_recorder);

    let generators: Vec<Box<dyn ModuleGenerator>> = vec![
        Box::new(TransportModuleGenerator),
        Box::new(ClientTraitModuleGenerator),
        Box::new(ResponseTypesModuleGenerator),
        Box::new(NodeManagerModuleGenerator),
        Box::new(TestNodeModuleGenerator),
        Box::new(LibRsModuleGenerator),
    ];

    for generator in generators.into_iter() {
        generator.generate_and_write(&ctx)?;
    }

    let crate_name = implementation.published_crate_name().to_string();
    feature_aware_cargo::generate_cargo_toml(out_dir, &ctx.rpc_methods, &crate_name, &version)?;
    Ok(())
}
