// SPDX-License-Identifier: CC0-1.0

//! Path utility functions for finding project roots and resolving paths.
//!
//! This module provides utilities for finding project roots, validating input paths,
//! loading registries, and resolving protocol specification paths.

use std::path::{Path, PathBuf};

use types::ProtocolVersion;

/// Find the workspace root by looking for the root Cargo.toml
///
/// This function walks up the directory tree from the current directory
/// until it finds a `Cargo.toml` file containing `[workspace]`.
///
/// # Returns
///
/// Returns `Result<PathBuf>` containing the path to the workspace root directory.
/// Returns an error if the workspace root cannot be found.
pub fn find_project_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut current = std::env::current_dir()?;
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            let contents = std::fs::read_to_string(&cargo_toml)?;
            if contents.contains("[workspace]") {
                return Ok(current);
            }
        }
        if !current.pop() {
            return Err("Could not find workspace root (no workspace Cargo.toml found)".into());
        }
    }
}

/// Get the protocol specification file path for a given protocol name
///
/// # Arguments
///
/// * `project_root` - Path to the project root directory
/// * `protocol_name` - Name of the protocol (e.g., "bitcoin")
///
/// # Returns
///
/// Returns `Result<PathBuf>` containing the path to the protocol specification file
pub fn get_protocol_spec_path(
    project_root: &Path,
    protocol_name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let spec_file = match protocol_name {
        "bitcoin" => "bitcoin-api.json",
        _ => {
            return Err(format!(
                "Unknown protocol '{}'. Supported protocols: bitcoin",
                protocol_name
            )
            .into());
        }
    };

    Ok(project_root.join("resources").join(spec_file))
}

/// Path to the canonical Bitcoin IR file
///
/// # Arguments
///
/// * `project_root` - Path to the project root directory
///
/// # Returns
///
/// Returns `PathBuf` for `project_root/resources/ir/bitcoin.ir.json`.
pub fn canonical_bitcoin_ir_path(project_root: &Path) -> PathBuf {
    project_root.join("resources/ir/bitcoin.ir.json")
}

/// Walk `levels_up` from a crate `CARGO_MANIFEST_DIR` to the workspace root.
///
/// Examples: adapters → 1, `compiler/codegen` → 2, `compiler/analysis` → 2.
pub fn workspace_root_from_manifest(manifest_dir: impl AsRef<Path>, levels_up: u32) -> PathBuf {
    let mut root = manifest_dir.as_ref().to_path_buf();
    for _ in 0..levels_up {
        if !root.pop() {
            break;
        }
    }
    root
}

/// Directory of RPC golden JSON fixtures.
pub fn rpc_golden_dir(project_root: &Path) -> PathBuf {
    project_root.join("resources/testdata/rpc_golden")
}

/// Path to one RPC golden fixture file under `resources/testdata/rpc_golden/`.
pub fn rpc_golden_path(project_root: &Path, filename: &str) -> PathBuf {
    rpc_golden_dir(project_root).join(filename)
}

/// Resolves IR output path for writing (relative paths are resolved against
/// project root).
///
/// Does not canonicalize; the file may not exist yet. Use this so read and
/// write use the same resolved path when targeting the canonical file.
///
/// # Arguments
///
/// * `project_root` - Path to the project root directory
/// * `output` - Requested output path (relative or absolute)
///
/// # Returns
///
/// Returns `PathBuf`: absolute `output` if `output` is absolute, else
/// `project_root.join(output)`.
pub fn resolve_ir_output_path(project_root: &Path, output: &Path) -> PathBuf {
    if output.is_absolute() {
        output.to_path_buf()
    } else {
        project_root.join(output)
    }
}

/// Get the path to the resources/ir directory relative to project root
///
/// # Returns
///
/// Returns `Result<PathBuf>` containing the path to the resources/ir directory
pub fn get_ir_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let project_root = find_project_root()?;
    Ok(project_root.join("resources/ir"))
}

/// Validate and resolve input file path using standard library methods
///
/// # Arguments
///
/// * `input_path` - Path to the input file (can be relative or absolute)
///
/// # Returns
///
/// Returns `Result<PathBuf>` containing the validated and resolved input path
pub fn validate_input_path(input_path: PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let project_root = find_project_root()?;

    // Resolve relative paths against project root, keep absolute paths as-is
    let resolved_path =
        if input_path.is_absolute() { input_path } else { project_root.join(input_path) };

    match resolved_path.canonicalize() {
        Ok(canonical_path) => Ok(canonical_path),
        Err(e) => Err(format!(
            "Input file not found: {:?}. Please provide a path to an API JSON file. Error: {}",
            resolved_path, e
        )
        .into()),
    }
}

/// Load and parse the registry.json file
///
/// # Returns
///
/// Returns `Result<serde_json::Value>` containing the parsed registry data
pub fn load_registry() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let project_root = find_project_root()?;
    let registry_path = project_root.join("resources/adapters/registry.json");

    let content = std::fs::read_to_string(&registry_path)
        .map_err(|e| format!("Failed to read registry.json: {}", e))?;

    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse registry.json: {}", e).into())
}

/// Format version string for filename (e.g., "30.2" -> "30_2_0", "30.2.1" -> "30_2_1")
///
/// Replaces dots with underscores to create filesystem-safe version strings.
/// Normalizes 2-part versions (e.g., "30.2") to 3-part versions with patch 0 (e.g., "30.2.0").
/// Uses `ProtocolVersion::as_filename_version()` for consistency across the codebase.
pub fn format_version_for_filename(version: &str) -> String {
    // Parse version string to ProtocolVersion for consistent formatting
    ProtocolVersion::from_string(version)
        .map(|v| {
            // If the original version string only has 2 parts, normalize to 3 parts with patch 0
            let parts: Vec<&str> = v.version_string.split('.').collect();
            if parts.len() == 2 {
                // Normalize 2-part version to 3-part: "30.2" -> "30.2.0" -> "30_2_0"
                format!("{}_{}_{}", v.major, v.minor, v.patch)
            } else {
                // Use the standard filename version for 3-part versions
                v.as_filename_version()
            }
        })
        .unwrap_or_else(|_| {
            let v = ProtocolVersion::from_string_for_ordering(version);
            format!("{}_{}_{}", v.major, v.minor, v.patch)
        })
}

/// Generate version-specific IR filename
///
/// # Arguments
///
/// * `version` - Version string (e.g., "30.2", "30.2.0", "30.2.1")
/// * `protocol` - Protocol name (e.g., "bitcoin")
///
/// # Examples
///
/// ```
/// use ethos_path::version_ir_filename;
/// assert_eq!(version_ir_filename("30.2", "bitcoin"), "v30_2_0_bitcoin.ir.json");
/// assert_eq!(version_ir_filename("30.2.0", "bitcoin"), "v30_2_0_bitcoin.ir.json");
/// assert_eq!(version_ir_filename("30.2.1", "bitcoin"), "v30_2_1_bitcoin.ir.json");
/// ```
pub fn version_ir_filename(version: &str, protocol: &str) -> String {
    format!("v{}_{}.ir.json", format_version_for_filename(version), protocol)
}
