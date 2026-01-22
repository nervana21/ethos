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
/// * `protocol_name` - Name of the protocol (e.g., "bitcoin", "lightning")
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
        "lightning" => "lightning-api.json",
        _ => {
            return Err(format!(
                "Unknown protocol '{}'. Supported protocols: bitcoin, lightning",
                protocol_name
            ).into());
        }
    };

    Ok(project_root.join("resources").join(spec_file))
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
        Err(e) => Err(format!("Input file not found: {:?}. Please provide a path to an API JSON file. Error: {}", resolved_path, e).into()),
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

/// Parse a version string into (major, minor, patch) components
///
/// Handles versions with or without 'v' prefix, and 2-part or 3-part versions
pub fn parse_version_components(version: &str) -> (u32, u32, u32) {
    let version_clean = version.trim_start_matches('v');
    let parts: Vec<&str> = version_clean.split('.').collect();
    let major: u32 = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch: u32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
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
            // Fallback: if parsing fails, use parsed components
            let (major, minor, patch) = parse_version_components(version);
            format!("{}_{}_{}", major, minor, patch)
        })
}

/// Generate version-specific IR filename
///
/// # Arguments
///
/// * `version` - Version string (e.g., "30.2", "30.2.0", "30.2.1")
/// * `protocol` - Protocol name (e.g., "bitcoin", "lightning")
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
