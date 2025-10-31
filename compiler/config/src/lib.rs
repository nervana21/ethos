#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Ethos Configuration
//!
//! This crate provides configuration management for Ethos.
//! It handles loading, saving, and managing configuration files that specify:
//! - Protocol connection settings (Bitcoin Core, Core Lightning, LND, etc.)
//! - Logging configuration
//! - Code generation parameters
//!
//! Configuration is stored in TOML format and can be loaded from files or created
//! with sensible defaults for development and testing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when loading or saving configuration
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Failed to read the configuration file from disk
    #[error("Failed to read config file: {0}")]
    FileRead(#[from] std::io::Error),
    /// Failed to parse the TOML configuration file
    #[error("Failed to parse config file: {0}")]
    Parse(#[from] toml::de::Error),
    /// Failed to serialize configuration to TOML format
    #[error("Failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),
    /// Configuration file was not found at the specified path
    #[error("Config file not found at: {0}")]
    NotFound(PathBuf),
    /// Could not locate the user's configuration directory
    #[error("Could not find user config directory")]
    ConfigDirUnavailable,
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Protocol connection settings (Bitcoin Core, Core Lightning, LND, etc.)
    pub protocol: ProtocolConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// Code generation settings
    pub codegen: CodegenConfig,
}

/// Protocol-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    /// Protocol type (e.g., "bitcoin_core", "core_lightning", "lnd")
    pub protocol_type: String,
    /// Protocol version (e.g., "v30.0.0", "v25.09.1")
    pub version: Option<String>,
    /// Network/chain configuration (regtest, testnet, mainnet, etc.)
    pub network: Option<String>,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (debug, info, warn, error)
    pub level: String,
    /// Log file path (optional)
    pub file: Option<PathBuf>,
}

/// Code generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodegenConfig {
    /// Path to the API schema file (resources/bitcoin-core-api.json)
    pub input_path: PathBuf,
    /// Where to write generated modules
    pub output_dir: PathBuf,
}

impl Config {
    /// Load configuration from a TOML file at `path`
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save this configuration as a pretty-printed TOML file at `path`
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Returns the default config file path:
    /// `{config_dir()}/ethos/config.toml`
    pub fn default_path() -> Result<PathBuf, ConfigError> {
        let config_dir = dirs::config_dir().ok_or(ConfigError::ConfigDirUnavailable)?.join("ethos");
        Ok(config_dir.join("config.toml"))
    }

    /// Get the default output directory for generated code
    pub fn default_output_dir() -> PathBuf {
        Self::default_output_dir_internal(
            std::env::var("OUT_DIR").ok(),
            std::env::current_dir().ok(),
        )
    }

    /// Internal function for testing - allows injection of environment values
    fn default_output_dir_internal(
        out_dir: Option<String>,
        current_dir: Option<PathBuf>,
    ) -> PathBuf {
        // First try to get OUT_DIR environment variable
        if let Some(out_dir) = out_dir {
            return PathBuf::from(out_dir);
        }

        // Fallback to current directory
        if let Some(current_dir) = current_dir {
            return current_dir;
        }

        // Last resort - use current directory as string
        PathBuf::from(".")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            protocol: ProtocolConfig {
                protocol_type: "bitcoin_core".to_string(),
                version: Some("v30.0.0".to_string()),
                network: Some("regtest".to_string()),
            },
            logging: LoggingConfig { level: "info".to_string(), file: None },
            codegen: CodegenConfig {
                input_path: PathBuf::from("resources/bitcoin-api.json"),
                output_dir: Self::default_output_dir(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn test_from_file() {
        // Test successful loading with explicit TOML content
        let temp_file = NamedTempFile::new().expect("Failed to create temporary file");
        let toml_content = r#"
            [protocol]
            protocol_type = "bitcoin_core"
            version = "v30.0.0"
            network = "regtest"

            [logging]
            level = "info"

            [codegen]
            input_path = "resources/bitcoin-api.json"
            output_dir = "generated"
        "#;
        fs::write(&temp_file, toml_content)
            .expect("Failed to write TOML content to temporary file");

        let loaded_config =
            Config::from_file(&temp_file).expect("Failed to load config from temporary file");
        assert_eq!(loaded_config.protocol.protocol_type, "bitcoin_core");
        assert_eq!(loaded_config.protocol.version, Some("v30.0.0".to_string()));
        assert_eq!(loaded_config.protocol.network, Some("regtest".to_string()));
        assert_eq!(loaded_config.logging.level, "info");
        assert_eq!(loaded_config.codegen.input_path, PathBuf::from("resources/bitcoin-api.json"));
        assert_eq!(loaded_config.codegen.output_dir, PathBuf::from("generated"));

        // Test successful parsing with different content
        let temp_file2 = NamedTempFile::new().expect("Failed to create second temporary file");
        let toml_content2 = r#"
            [protocol]
            protocol_type = "core_lightning"
            version = "v25.09.1"
            network = "testnet"

            [logging]
            level = "debug"
            file = "debug.log"

            [codegen]
            input_path = "test_api.json"
            output_dir = "test_generated"
        "#;
        fs::write(&temp_file2, toml_content2)
            .expect("Failed to write second TOML content to temporary file");

        let loaded_config2 = Config::from_file(&temp_file2)
            .expect("Failed to load second config from temporary file");
        assert_eq!(loaded_config2.protocol.protocol_type, "core_lightning");
        assert_eq!(loaded_config2.protocol.version, Some("v25.09.1".to_string()));
        assert_eq!(loaded_config2.protocol.network, Some("testnet".to_string()));
        assert_eq!(loaded_config2.logging.level, "debug");
        assert_eq!(loaded_config2.logging.file, Some(PathBuf::from("debug.log")));
        assert_eq!(loaded_config2.codegen.input_path, PathBuf::from("test_api.json"));
        assert_eq!(loaded_config2.codegen.output_dir, PathBuf::from("test_generated"));

        // Test file not found error
        let result = Config::from_file("nonexistent_file.toml");
        assert!(result.is_err());
        match result.expect_err("Expected error for nonexistent file") {
            ConfigError::FileRead(_) => {}
            _ => panic!("Expected FileRead error"),
        }

        // Test parse error
        let temp_file =
            NamedTempFile::new().expect("Failed to create temporary file for parse error test");
        fs::write(&temp_file, "invalid toml content")
            .expect("Failed to write invalid TOML content");

        let result = Config::from_file(&temp_file);
        assert!(result.is_err());
        match result.expect_err("Expected parse error for invalid TOML") {
            ConfigError::Parse(_) => {}
            _ => panic!("Expected Parse error"),
        }
    }

    #[test]
    fn test_save() {
        let config = Config::default();
        let temp_file =
            NamedTempFile::new().expect("Failed to create temporary file for save test");

        // Test successful save
        let result = config.save(&temp_file);
        assert!(result.is_ok());

        // Verify the file was written and can be read back
        let contents = fs::read_to_string(&temp_file).expect("Failed to read saved config file");
        assert!(contents.contains("bitcoin_core"));
        assert!(contents.contains("v30.0.0"));
        assert!(contents.contains("regtest"));
        assert!(contents.contains("info"));

        // Test file write error - try to save to a non-existent directory
        let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
        let non_existent_subdir = temp_dir.path().join("nonexistent").join("config.toml");

        let result = config.save(&non_existent_subdir);
        assert!(result.is_err());

        // Verify the error is a FileRead error (from std::fs::write)
        match result.expect_err("Expected file write error for non-existent directory") {
            ConfigError::FileRead(_) => (), // Expected
            other => panic!("Expected FileRead error, got {:?}", other),
        }
    }

    #[test]
    fn test_default_path() {
        let path = Config::default_path().expect("Failed to get default config path");
        assert!(path.to_str().expect("Path should be valid UTF-8").ends_with("ethos/config.toml"));

        // Test that the path contains the expected directory structure
        let path_str = path.to_str().expect("Path should be valid UTF-8");
        assert!(path_str.contains("ethos"));
        assert!(path_str.ends_with("config.toml"));
    }

    #[test]
    fn test_default_output_dir() {
        let dir = Config::default_output_dir();

        // Test that we get a valid path
        assert!(dir.to_str().is_some());

        // Test OUT_DIR environment variable path
        std::env::set_var("OUT_DIR", "/tmp/test_out_dir");
        let dir_with_env = Config::default_output_dir();
        assert_eq!(dir_with_env.to_str().expect("Path should be valid UTF-8"), "/tmp/test_out_dir");

        // Clean up
        std::env::remove_var("OUT_DIR");

        // Test fallback to current directory
        std::env::remove_var("OUT_DIR");
        let temp_dir =
            tempfile::tempdir().expect("Failed to create temporary directory for current dir test");
        let original_dir = std::env::current_dir().expect("Failed to get current directory");
        std::env::set_current_dir(&temp_dir).expect("Failed to change to temporary directory");
        let dir = Config::default_output_dir();
        let canonical_temp_path = temp_dir
            .path()
            .canonicalize()
            .expect("Failed to canonicalize temporary directory path");
        assert_eq!(dir, canonical_temp_path);
        std::env::set_current_dir(original_dir).expect("Failed to restore original directory");

        // Test last resort fallback
        std::env::remove_var("OUT_DIR");
        let dir = Config::default_output_dir();
        assert!(dir.to_str().is_some());
        assert!(!dir.to_str().expect("Path should be valid UTF-8").is_empty());
    }

    #[test]
    fn test_default_output_dir_internal() {
        // Test OUT_DIR takes precedence
        let dir = Config::default_output_dir_internal(
            Some("/tmp/out_dir".to_string()),
            Some(PathBuf::from("/tmp/current")),
        );
        assert_eq!(dir, PathBuf::from("/tmp/out_dir"));

        // Test current_dir fallback when OUT_DIR is None
        let dir = Config::default_output_dir_internal(None, Some(PathBuf::from("/tmp/current")));
        assert_eq!(dir, PathBuf::from("/tmp/current"));

        // Test the fallback case by passing None for both parameters
        let dir = Config::default_output_dir_internal(None, None);
        assert_eq!(dir, PathBuf::from("."));
    }

    #[test]
    fn test_default() {
        let config = Config::default();
        assert_eq!(config.protocol.protocol_type, "bitcoin_core");
        assert_eq!(config.protocol.version, Some("v30.0.0".to_string()));
        assert_eq!(config.protocol.network, Some("regtest".to_string()));
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.logging.file, None);
        assert_eq!(config.codegen.input_path, PathBuf::from("resources/bitcoin-api.json"));
        // output_dir is dynamic, so we just check it's not empty
        assert!(!config
            .codegen
            .output_dir
            .to_str()
            .expect("Output directory path should be valid UTF-8")
            .is_empty());
    }
}
