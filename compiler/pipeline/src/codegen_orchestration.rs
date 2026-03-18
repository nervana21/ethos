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
use codegen::utils::{validate_method_mappings, SuggestedMapping};
use ir::ProtocolIR;
use path::find_project_root;
use serde_json::Value;
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

/// Writes suggested method mappings into both normalization JSON files under `workspace_root`.
/// Inserts only new keys, each at its alphabetical position; existing key order is preserved.
fn apply_suggested_mappings(
    workspace_root: &Path,
    implementation: Implementation,
    suggestions: &[SuggestedMapping],
) -> Result<(), PipelineError> {
    let impl_key = implementation.as_str();
    let filename = implementation.protocol_name();
    let paths: Vec<_> = normalization::NORMALIZATION_JSON_DIRS
        .iter()
        .map(|dir| workspace_root.join(dir).join(format!("{filename}.json")))
        .collect();

    for path in &paths {
        if !path.exists() {
            return Err(PipelineError::Message(format!(
                "Normalization file not found: {}",
                path.display()
            )));
        }
        let contents = fs::read_to_string(path)?;
        let mut data: Value = serde_json::from_str(&contents).map_err(|e| {
            PipelineError::Message(format!("Invalid JSON in {}: {}", path.display(), e))
        })?;
        let mappings = data
            .get_mut("method_mappings")
            .and_then(|m| m.get_mut(impl_key))
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| {
                PipelineError::Message(format!(
                    "Missing method_mappings.{} in {}",
                    impl_key,
                    path.display()
                ))
            })?;
        // New entries only, sorted by key so we can merge in alphabetical position
        let mut new_entries: Vec<_> = suggestions
            .iter()
            .filter(|s| !mappings.contains_key(&s.suggested_key))
            .map(|s| (s.suggested_key.clone(), Value::String(s.rpc_method.clone())))
            .collect();
        if new_entries.is_empty() {
            continue;
        }
        new_entries.sort_by(|a, b| a.0.cmp(&b.0));
        // Preserve existing order; insert each new key at its alphabetical position
        let existing: Vec<_> = mappings.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let mut merged = Vec::with_capacity(existing.len() + new_entries.len());
        let mut j = 0;
        for (k, v) in existing {
            while j < new_entries.len() && new_entries[j].0 < k {
                merged.push(new_entries[j].clone());
                j += 1;
            }
            merged.push((k, v));
        }
        for (k, v) in new_entries.into_iter().skip(j) {
            merged.push((k, v));
        }
        mappings.clear();
        for (k, v) in merged {
            mappings.insert(k, v);
        }
        let mut out = serde_json::to_string_pretty(&data)
            .map_err(|e| PipelineError::Message(format!("JSON serialize: {}", e)))?;
        if !out.ends_with('\n') {
            out.push('\n');
        }
        fs::write(path, out)?;
    }
    Ok(())
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

    // Validate method mappings before generating. If any are missing, write suggestions
    // into both normalization JSON files and ask the user to re-run.
    if let Err(e) = validate_method_mappings(implementation.as_str(), &rpc_methods) {
        let workspace_root =
            find_project_root().map_err(|err| PipelineError::Message(err.to_string()))?;
        apply_suggested_mappings(&workspace_root, implementation, &e.suggestions)?;
        let n = e.suggestions.len();
        return Err(PipelineError::Message(format!(
            "Suggested mapping(s) for {} unmapped RPC method(s) have been written to both \
             normalization JSON files. Review the changes before committing, \
             then re-run the same command to continue.",
            n
        )));
    }

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
