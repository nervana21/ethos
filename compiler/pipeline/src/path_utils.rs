//! Path and registry utility functions for the pipeline.
//!
//! This module provides utilities for finding project roots, validating input paths,
//! loading registries, and resolving protocol specification paths.

use std::path::{Path, PathBuf};
use std::{env, fs};

use crate::PipelineError;

/// Find the workspace root by looking for the root Cargo.toml
///
/// # Returns
///
/// Returns `Result<PathBuf>` containing the path to the workspace root directory
pub fn find_project_root() -> Result<PathBuf, PipelineError> {
	let mut current = env::current_dir()?;
	loop {
		let cargo_toml = current.join("Cargo.toml");
		if cargo_toml.exists() {
			// Read the Cargo.toml to check if it's the workspace root
			let contents = fs::read_to_string(&cargo_toml)?;
			if contents.contains("[workspace]") {
				return Ok(current);
			}
		}
		if !current.pop() {
			return Err(PipelineError::Message(
				"Could not find workspace root (no workspace Cargo.toml found)".to_string(),
			));
		}
	}
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
pub fn validate_input_path(input_path: PathBuf) -> Result<PathBuf, PipelineError> {
	let project_root = find_project_root()?;

	// Resolve relative paths against project root, keep absolute paths as-is
	let resolved_path =
		if input_path.is_absolute() { input_path } else { project_root.join(input_path) };

	match resolved_path.canonicalize() {
		Ok(canonical_path) => Ok(canonical_path),
		Err(_) => Err(PipelineError::Message(format!(
			"Input file not found: {:?}. Please provide a path to an API JSON file.",
			resolved_path
		))),
	}
}

/// Load and parse the registry.json file
///
/// # Returns
///
/// Returns `Result<serde_json::Value>` containing the parsed registry data
pub fn load_registry() -> Result<serde_json::Value, PipelineError> {
	let project_root = find_project_root()?;
	let registry_path = project_root.join("resources/adapters/registry.json");

	let content = fs::read_to_string(&registry_path)
		.map_err(|e| PipelineError::Message(format!("Failed to read registry.json: {}", e)))?;

	serde_json::from_str(&content)
		.map_err(|e| PipelineError::Message(format!("Failed to parse registry.json: {}", e)))
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
	project_root: &Path, protocol_name: &str,
) -> Result<PathBuf, PipelineError> {
	let spec_file = match protocol_name {
		"bitcoin" => "bitcoin-api.json",
		"lightning" => "lightning-api.json",
		_ => {
			return Err(PipelineError::Message(format!(
				"Unknown protocol '{}'. Supported protocols: bitcoin, lightning",
				protocol_name
			)))
		},
	};

	Ok(project_root.join("resources").join(spec_file))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_find_project_root() {
		let project_root = find_project_root().expect("Failed to find project root");

		assert!(project_root.exists());
		assert!(project_root.is_dir());

		let cargo_toml = project_root.join("Cargo.toml");
		assert!(cargo_toml.exists());

		let contents = fs::read_to_string(&cargo_toml).expect("Failed to read Cargo.toml");
		assert!(contents.contains("[workspace]"));
	}
}
