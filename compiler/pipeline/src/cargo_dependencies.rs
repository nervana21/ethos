//! Single source of truth for dependencies in generated client crates (e.g. ethos-bitcoind).
//!
//! Used by both project_setup (scaffold Cargo.toml) and feature_aware_cargo (final Cargo.toml)
//! so dependency list and versions stay DRY and security pins apply everywhere.
//!
//! Generated Cargo.toml does not include `[workspace]`; package section and deps are built here.

/// Format the `[package]` section for generated crates (no `[workspace]`).
/// Callers supply name, version, description, and authors so scaffold and final Cargo.toml can differ.
pub fn format_package_section(
    name: &str,
    version: &str,
    description: &str,
    authors: &str,
) -> String {
    format!(
        r#"[package]
publish = true

name = "{}"
version = "{}"
edition = "2021"
authors = ["{}"]
license = "CC0-1.0"
description = "{}"
readme = "README.md"
keywords = ["bitcoin", "protocol", "compiler", "integration-testing"]
categories = ["cryptography", "data-structures", "api-bindings"]
repository = "https://github.com/nervana21/ethos"
homepage = "https://github.com/nervana21/ethos"
documentation = "https://docs.rs/{}"

"#,
        name, version, authors, description, name
    )
}

/// The `[dependencies]` section for generated client crates.
///
/// Types-only consumers enable no `client` feature and skip tokio/tempfile/transport deps.
/// Keep in sync with any RUSTSEC pins (e.g. bytes >=1.11.1 for RUSTSEC-2026-0007).
pub const GENERATED_CRATE_DEPENDENCIES: &str = r#"[dependencies]
bitcoin = { version = "0.32.8", default-features = false, features = ["std", "serde"] }
bitcoin-units = { version = "0.3.0", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0.145", features = ["preserve_order"] }

# Optional client / node-manager stack (feature = "client")
async-trait = { version = "0.1.89", optional = true }
base64 = { version = "0.22", optional = true }
bitreq = { version = "0.3.7", default-features = false, features = ["async-https"], optional = true }
bytes = { version = ">=1.11.1", optional = true }  # RUSTSEC-2026-0007: Integer overflow in BytesMut::reserve (tokio transitive)
tempfile = { version = "3.23.0", optional = true }
thiserror = { version = "2.0.17", optional = true }
tokio = { version = "1.49", features = ["io-util", "macros", "net", "process", "rt", "rt-multi-thread", "sync", "time"], optional = true }
tracing = { version = "0.1.41", optional = true }
"#;
