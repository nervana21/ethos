#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Ethos Plugins
//!
//! This crate defines the plugin system for extending Ethos with custom
//! adapters, passes, and transformations.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Plugin trait for extending compiler functionality
pub trait Plugin: Send + Sync {
    /// Plugin name
    fn name(&self) -> &'static str;

    /// Plugin description
    fn description(&self) -> &'static str { "" }
}

/// Specialization for protocol adapters
pub trait AdapterPlugin: Plugin {
    /// Extract protocol IR from a schema file
    fn extract_protocol_ir(
        &self,
        path: &Path,
    ) -> Result<ir::ProtocolIR, Box<dyn std::error::Error + Send + Sync>>;
}

/// Plugin error type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginError {
    /// Plugin initialization failed
    InitializationFailed(String),
    /// Plugin execution failed
    ExecutionFailed(String),
    /// Plugin configuration error
    ConfigurationError(String),
    /// Plugin dependency error
    DependencyError(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::InitializationFailed(msg) => write!(f, "Initialization failed: {}", msg),
            PluginError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            PluginError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            PluginError::DependencyError(msg) => write!(f, "Dependency error: {}", msg),
        }
    }
}

impl std::error::Error for PluginError {}

use std::collections::HashMap;

/// Plugin registry for managing plugins
pub struct PluginRegistry {
    adapters: HashMap<String, Box<dyn AdapterPlugin>>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self { Self { adapters: HashMap::new() } }

    /// Register an adapter plugin
    pub fn register_adapter(&mut self, adapter: Box<dyn AdapterPlugin>) {
        self.adapters.insert(adapter.name().to_string(), adapter);
    }

    /// Get an adapter by name
    pub fn get_adapter(&self, name: &str) -> Option<&dyn AdapterPlugin> {
        self.adapters.get(name).map(|a| &**a)
    }

    /// List all registered adapters
    pub fn list_adapters(&self) -> Vec<&str> { self.adapters.keys().map(|s| s.as_str()).collect() }
}

impl Default for PluginRegistry {
    fn default() -> Self { Self::new() }
}
