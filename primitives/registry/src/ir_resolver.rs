//! IR File Resolution
//!
//! This module provides a unified way to resolve IR file paths based on the adapter registry.
//! It eliminates hardcoded IR file paths throughout the codebase.

use std::path::PathBuf;

use path::find_project_root;
use serde_json::Value;
use types::implementation::{Implementation, Protocol};

/// Errors that can occur during IR file resolution
#[derive(Debug, thiserror::Error)]
pub enum IrResolverError {
    /// I/O error while reading files
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON parsing error
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Registry structure error
    #[error("{0}")]
    Registry(String),
    /// IR file not found
    #[error("IR file not found for {0}")]
    NotFound(String),
}

/// Result alias for IR resolver operations
pub type IrResolverResult<T> = std::result::Result<T, IrResolverError>;

/// IR file resolver that uses the adapter registry to locate IR files
pub struct IrResolver {
    registry: Value,
    project_root: PathBuf,
}

impl IrResolver {
    /// Create a new IR resolver by loading the adapter registry
    pub fn new() -> IrResolverResult<Self> {
        let project_root = Self::find_project_root()?;
        let registry_path = project_root.join("resources/adapters/registry.json");

        let content = std::fs::read_to_string(&registry_path)?;
        let registry: Value = serde_json::from_str(&content)?;

        Ok(Self { registry, project_root })
    }

    /// Find the workspace root by looking for the root Cargo.toml
    fn find_project_root() -> IrResolverResult<PathBuf> {
        find_project_root().map_err(|e| IrResolverError::Registry(e.to_string()))
    }

    /// Resolve the IR file path for a given protocol
    pub fn resolve_ir_path(&self, protocol: &Protocol) -> IrResolverResult<PathBuf> {
        let protocol_name = protocol.as_str();

        let ir_file =
            self.registry["adapters"][protocol_name]["ir_file"].as_str().ok_or_else(|| {
                IrResolverError::Registry(format!(
                    "No ir_file found for protocol: {}",
                    protocol_name
                ))
            })?;

        Ok(self.project_root.join(ir_file))
    }

    /// Resolve the IR file path for a given implementation
    pub fn resolve_ir_path_for_implementation(
        &self,
        implementation: &Implementation,
    ) -> IrResolverResult<PathBuf> {
        let protocol_name = implementation.protocol_name();
        let protocol = protocol_name
            .parse::<Protocol>()
            .map_err(|e| IrResolverError::Registry(format!("Invalid protocol name: {}", e)))?;
        self.resolve_ir_path(&protocol)
    }

    /// Returns the default version string for this implementation, as defined in the registry.
    ///
    /// This reads `registry["adapters"][protocol]["dialects"][implementation]["default_version"]`
    /// (e.g. for Bitcoin Core this might be `"v30.2"`). The string is defined per adapter and may
    /// mean a protocol version, a crate version, or a release tag—callers treat it as an opaque
    /// version identifier.
    ///
    /// Call this when the user did not pass `--version`: use the returned value as the version to
    /// run.
    pub fn default_version_for_implementation(
        &self,
        implementation: &Implementation,
    ) -> IrResolverResult<String> {
        let protocol_name = implementation.protocol_name();
        let dialect_key = implementation.as_str();
        let version = self.registry["adapters"][protocol_name]["dialects"][dialect_key]
            ["default_version"]
            .as_str()
            .ok_or_else(|| {
                IrResolverError::Registry(format!(
                    "No default_version for implementation '{}' in registry",
                    dialect_key
                ))
            })?;
        Ok(version.to_string())
    }

    /// Get the protocol name for a given implementation
    pub fn get_protocol_for_implementation(
        &self,
        implementation: &Implementation,
    ) -> IrResolverResult<Protocol> {
        let protocol_name = implementation.protocol_name();
        protocol_name
            .parse::<Protocol>()
            .map_err(|e| IrResolverError::Registry(format!("Invalid protocol name: {}", e)))
    }

    /// Check if an IR file exists for a given protocol
    pub fn ir_file_exists(&self, protocol: &Protocol) -> bool {
        match self.resolve_ir_path(protocol) {
            Ok(path) => path.exists(),
            Err(_) => false,
        }
    }

    /// Check if an IR file exists for a given implementation
    pub fn ir_file_exists_for_implementation(&self, implementation: &Implementation) -> bool {
        match self.resolve_ir_path_for_implementation(implementation) {
            Ok(path) => path.exists(),
            Err(_) => false,
        }
    }

    /// List all available protocols that have IR files
    pub fn list_available_protocols(&self) -> IrResolverResult<Vec<Protocol>> {
        let mut protocols = Vec::new();

        if let Some(adapters) = self.registry["adapters"].as_object() {
            for (protocol_name, _) in adapters {
                if let Ok(protocol) = protocol_name.parse::<Protocol>() {
                    if self.ir_file_exists(&protocol) {
                        protocols.push(protocol);
                    }
                }
            }
        }

        Ok(protocols)
    }
}

impl Default for IrResolver {
    fn default() -> Self { Self::new().expect("Failed to create IR resolver") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_ir_paths() {
        let resolver = IrResolver::new().expect("Failed to create resolver");

        // Test protocol resolution
        let bitcoin_path = resolver
            .resolve_ir_path(&Protocol::Bitcoin)
            .expect("Failed to resolve Bitcoin IR path");
        assert!(bitcoin_path.to_string_lossy().contains("bitcoin.ir.json"));

        // Test implementation resolution
        let bitcoin_core_path = resolver
            .resolve_ir_path_for_implementation(&Implementation::BitcoinCore)
            .expect("Failed to resolve Bitcoin Core IR path");
        assert!(bitcoin_core_path.to_string_lossy().contains("bitcoin.ir.json"));
    }

    #[test]
    fn test_list_available_protocols() {
        let resolver = IrResolver::new().expect("Failed to create resolver");
        let protocols = resolver.list_available_protocols().expect("Failed to list protocols");

        assert!(protocols.contains(&Protocol::Bitcoin));
    }
}
