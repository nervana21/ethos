//! Node metadata for Bitcoin protocol implementations.
//!
//! This module provides metadata structures that define how to spawn and manage
//! nodes for different Bitcoin protocol implementations during testing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Metadata for node management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Binary executable name (e.g., "bitcoind", "lightningd")
    pub executable: String,
    /// Transport protocol ("http" or "unix")
    pub transport: String,
    /// Whether authentication is required
    pub requires_auth: bool,
    /// CLI argument templates
    pub cli_args: CliArgs,
    /// Readiness check method name
    pub readiness_method: String,
    /// Error codes that indicate initialization is in progress
    pub initialization_error_codes: Vec<i32>,
    /// Socket path pattern (for Unix socket transports)
    pub socket_path_pattern: Option<String>,
}

/// CLI argument configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliArgs {
    /// Arguments that take values (format: "arg_name" -> "arg_template")
    pub value_args: HashMap<String, String>,
    /// Static arguments that don't take values
    pub static_args: Vec<String>,
}

impl CliArgs {
    /// Create a new CLI args configuration
    pub fn new() -> Self { Self { value_args: HashMap::new(), static_args: Vec::new() } }

    /// Add a value argument
    pub fn add_value_arg(mut self, name: &str, template: &str) -> Self {
        self.value_args.insert(name.to_string(), template.to_string());
        self
    }

    /// Add a static argument
    pub fn add_static_arg(mut self, arg: &str) -> Self {
        self.static_args.push(arg.to_string());
        self
    }
}

impl Default for CliArgs {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let args = CliArgs::new();
        assert!(args.value_args.is_empty());
        assert!(args.static_args.is_empty());
    }

    #[test]
    fn test_add_value_arg() {
        let args = CliArgs::new().add_value_arg("name", "template");
        assert_eq!(args.value_args.get("name"), Some(&"template".to_string()));
        assert!(args.static_args.is_empty());
    }

    #[test]
    fn test_add_static_arg() {
        let args = CliArgs::new().add_static_arg("--flag");
        assert_eq!(args.static_args, vec!["--flag".to_string()]);
        assert!(args.value_args.is_empty());
    }
}
