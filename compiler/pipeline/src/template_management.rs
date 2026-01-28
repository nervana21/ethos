//! Template management for the pipeline.
//!
//! This module handles template file operations and source directory creation.

use std::fs;
use std::path::{Path, PathBuf};

use path::find_project_root;
use types::Implementation;

use crate::PipelineError;

/// Template files to be copied to the generated crate
const TEMPLATE_FILES: &[&str] = &["config.rs", "test_config.rs"];

/// Create source directory structure and copy template files
///
/// # Arguments
///
/// * `crate_root` - Path to the crate root directory
/// * `implementation` - Name of the implementation
///
/// # Returns
///
/// Returns `Result<PathBuf>` containing the path to the src directory
pub fn create_source_directory_with_templates(
    crate_root: &Path,
    implementation: Implementation,
) -> Result<PathBuf, PipelineError> {
    let src_dir = crate_root.join("src");
    fs::create_dir_all(&src_dir)?;

    copy_templates_to(&src_dir, implementation)?;

    Ok(src_dir)
}

/// Copy template files to the destination directory
///
/// # Arguments
///
/// * `dst_dir` - The destination directory for the template files
/// * `implementation` - The implementation name (e.g., "bitcoin_core", "core_lightning")
///
/// # Returns
///
/// Returns `Result<()>` indicating success or failure of copying the template files
pub fn copy_templates_to(
    dst_dir: &Path,
    implementation: Implementation,
) -> Result<(), PipelineError> {
    let project_root = find_project_root().map_err(|e| PipelineError::Message(e.to_string()))?;
    let src_dir = project_root.join(format!("adapters/templates/{}", implementation));

    // Error if implementation-specific templates do not exist
    if !src_dir.exists() {
        return Err(PipelineError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Template directory for implementation '{}' does not exist", implementation),
        )));
    }
    let template_dir = src_dir;

    for filename in TEMPLATE_FILES {
        let src_path = template_dir.join(filename);
        let dst_path = dst_dir.join(filename);
        fs::copy(&src_path, &dst_path)?;
    }

    Ok(())
}
