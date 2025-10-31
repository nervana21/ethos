//! Feature-aware Cargo.toml generation
//!
//! Generates Cargo.toml with feature flags based on method categories

use std::path::Path;

use ir::RpcDef;
use semantics::method_categorization::{group_methods_by_category, MethodCategory};
use types::ProtocolVersion;

use crate::PipelineError;

/// Generate Cargo.toml with feature flags
pub fn generate_cargo_toml(
    output_dir: &Path,
    methods: &[RpcDef],
    crate_name: &str,
    version: &ProtocolVersion,
) -> Result<(), PipelineError> {
    // Validate input early
    if methods.is_empty() {
        return Err(PipelineError::Message(
			"No RPC methods provided for code generation. This indicates a problem with the input data or version configuration.".to_string()
		));
    }

    let groups = group_methods_by_category(methods);

    let mut cargo_content = String::new();

    // Basic package info
    cargo_content.push_str(&format!(
        r#"[package]
publish = true

name = "{}"
version = "{}"
edition = "2021"
authors = ["Ethos Developers"]
license = "CC0 1.0"
description = "Generated client for {}."
readme = "README.md"
keywords = ["bitcoin", "protocol", "compiler", "integration-testing"]
categories = ["cryptography", "data-structures", "api-bindings"]
repository = "https://github.com/nervana21/ethos"
homepage = "https://github.com/nervana21/ethos"
documentation = "https://docs.rs/ethos-spec"

"#,
        crate_name,
        version.crate_version(),
        crate_name
    ));

    cargo_content.push_str("[workspace]\n\n");

    // Dependencies (same as current)
    cargo_content.push_str(
        r#"[dependencies]
async-trait = "0.1.89"
bitcoin = { version = "0.32.8", features = ["rand", "serde"] }
http = { package = "ethos-http", path = "../../../backends/http" }
reqwest = { version = "0.12.26", default-features = false, features = [
    "json",
    "rustls-tls",
] }
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0.145", features = ["preserve_order"] }
tempfile = "3.23.0"
thiserror = "2.0.17"
tokio = { version = "1", features = ["full"] }
tracing = "0.1.41"
transport = { package = "ethos-transport", path = "../../../primitives/transport" }
types = { package = "ethos-types", path = "../../../primitives/types" }

"#,
    );

    cargo_content.push_str("\n[features]\n");

    // Generate default features based on available categories
    let mut default_categories: Vec<MethodCategory> =
        groups.keys().filter(|&c| c.is_default()).cloned().collect();
    default_categories.sort_by_key(|c| c.display_name());
    let default_features: Vec<&str> = default_categories.iter().map(|c| c.feature_name()).collect();

    if default_features.is_empty() {
        if groups.is_empty() {
            return Err(PipelineError::Message(
				"No RPC methods found for the specified protocol version. This indicates a problem with the input data or version configuration.".to_string()
			));
        }

        return Err(PipelineError::Message(format!(
			"No core features (blockchain, network, util, rawtransaction) found in {} methods. This may indicate a data quality issue or unsupported protocol version. Available categories: {}",
			groups.values().map(|v| v.len()).sum::<usize>(),
			groups.keys().map(|c| c.feature_name()).collect::<Vec<_>>().join(", ")
		)));
    }

    cargo_content.push_str(&format!("default = [\"{}\"]\n", default_features.join("\", \"")));

    // Emit feature flags
    let mut categories: Vec<MethodCategory> = groups.keys().cloned().collect();
    categories.sort_by_key(|c| c.display_name());
    for category in categories.iter() {
        let feature_name = category.feature_name();
        cargo_content.push_str(&format!("{} = []\n", feature_name));
    }

    cargo_content.push_str("\n# Enable all features\n");
    let mut all_features: Vec<String> =
        groups.keys().map(|c| c.feature_name().to_string()).collect();
    all_features.sort();
    cargo_content.push_str(&format!("full = [\"{}\"]\n", all_features.join("\", \"")));

    // Add serde-deny-unknown-fields feature
    cargo_content.push_str("serde-deny-unknown-fields = []\n");

    let cargo_path = output_dir
        .parent()
        .ok_or_else(|| {
            PipelineError::Message("Invalid output directory: no parent directory".to_string())
        })?
        .join("Cargo.toml");
    std::fs::write(cargo_path, cargo_content)?;

    Ok(())
}
