//! Pipeline orchestration for the main entry points.
//!
//! This module contains the main pipeline entry points that coordinate all the modules
//! to execute the complete code generation process.

use std::fs;
use std::path::PathBuf;

use path::{find_project_root, load_registry};
use registry::ir_resolver::IrResolver;
use types::{Implementation, ProtocolVersion};

use crate::codegen_orchestration::{analyze_implementation, generate_into};
use crate::project_setup::setup_project_files;
use crate::protocol_compiler::EthosCompiler;
use crate::template_management::create_source_directory_with_templates;
use crate::PipelineError;

/// Run the pipeline for all implementations with their default versions.
///
/// This function reads the registry.json to discover all available protocol implementations
/// and generates client libraries for each one using their default versions.
///
/// # Returns
///
/// Returns `Result<()>` indicating success or failure of the generation process
pub fn run_all() -> Result<(), PipelineError> {
    let registry = load_registry().map_err(|e| PipelineError::Message(e.to_string()))?;
    let _project_root = find_project_root().map_err(|e| PipelineError::Message(e.to_string()))?;

    // Iterate through all protocols and their dialects
    for (protocol_name, protocol_info) in registry["adapters"].as_object().ok_or_else(|| {
        PipelineError::Message("Invalid registry: missing 'adapters' field".to_string())
    })? {
        let dialects = protocol_info["dialects"].as_object().ok_or_else(|| {
            PipelineError::Message(format!(
                "Invalid registry: no dialects for protocol '{}'",
                protocol_name
            ))
        })?;

        for (implementation, dialect_info) in dialects {
            // Only process implementations that have working adapters
            let implementation = match implementation.parse::<Implementation>() {
                Ok(impl_name) => impl_name,
                Err(_) => {
                    continue;
                }
            };

            // Only process implementations that have working adapters
            let supported_implementations =
                [Implementation::BitcoinCore, Implementation::CoreLightning];
            if !supported_implementations.contains(&implementation) {
                continue;
            }

            let default_version = dialect_info["default_version"].as_str().ok_or_else(|| {
                PipelineError::Message(format!(
                    "Invalid registry: no default_version for '{}'",
                    implementation
                ))
            })?;

            let version = ProtocolVersion::from_string_with_protocol(
                default_version,
                Some(implementation.to_string()),
            )
            .expect("Failed to parse version");
            compile_from_ir(implementation, &version, None)?;
        }
    }

    Ok(())
}

/// Compile a protocol using IR files directly
///
/// # Arguments
///
/// * `implementation` - Name of the implementation (e.g., "bitcoin_core", "core_lightning").
/// * `version` - Version string for the API (e.g., "v30.0", "v25.09.1").
/// * `crate_root` - Optional output directory for the generated crate. If None, uses default naming convention.
///
/// # Returns
///
/// Returns `Result<()>` indicating success or failure of the generation process
pub fn compile_from_ir(
    implementation: Implementation,
    version: &ProtocolVersion,
    crate_root: Option<PathBuf>,
) -> Result<(), PipelineError> {
    let crate_root = match crate_root {
        Some(path) => path,
        None => {
            let project_root =
                find_project_root().map_err(|e| PipelineError::Message(e.to_string()))?;
            // Use {published_crate_name} format for directory naming
            let generated_path = project_root
                .join(format!("outputs/generated/{}", implementation.published_crate_name()));
            generated_path
        }
    };

    if crate_root.exists() {
        fs::remove_dir_all(&crate_root)?;
    }

    // Create source directory structure and copy template files
    let src_dir = create_source_directory_with_templates(&crate_root, implementation)?;

    // Load ProtocolIR directly from IR file using registry
    let ir_resolver = IrResolver::new()
        .map_err(|e| PipelineError::Message(format!("Failed to create IR resolver: {}", e)))?;
    let ir_path = ir_resolver.resolve_ir_path_for_implementation(&implementation).map_err(|e| {
        PipelineError::Message(format!("Failed to resolve IR path for {}: {}", implementation, e))
    })?;

    let mut protocol_ir = ir::ProtocolIR::from_file(&ir_path)
        .map_err(|e| PipelineError::Message(format!("Failed to load IR file: {}", e)))?;

    // Run compiler passes (validation, canonicalization, etc.)
    let compiler = EthosCompiler::new();
    protocol_ir = compiler.run_compiler_passes(protocol_ir, &crate_root)?;

    // Setup project files (Cargo.toml, README, etc.)
    setup_project_files(&crate_root, version, implementation)?;

    // Run semantic analysis on the IR
    let compiler_ctx = analyze_implementation(implementation, protocol_ir, version, ir_path)?;

    // Generate code
    generate_into(&src_dir, &compiler_ctx)?;

    Ok(())
}
