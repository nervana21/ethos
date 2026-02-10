//! Single source of truth for dependencies in generated client crates (e.g. ethos-bitcoind).
//!
//! Used by both project_setup (scaffold Cargo.toml) and feature_aware_cargo (final Cargo.toml)
//! so dependency list and versions stay DRY and security pins apply everywhere.

/// The `[dependencies]` section for generated client crates.
/// Keep in sync with any RUSTSEC pins (e.g. bytes >=1.11.1 for RUSTSEC-2026-0007).
pub const GENERATED_CRATE_DEPENDENCIES: &str = r#"[dependencies]
async-trait = "0.1.89"
base64 = "0.22"
bitcoin = { version = "0.32.8", features = ["rand", "serde"] }
bitreq = { version = "0.3.1", default-features = false, features = ["async-https"] }
bytes = ">=1.11.1"  # RUSTSEC-2026-0007: Integer overflow in BytesMut::reserve (tokio transitive)
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0.145", features = ["preserve_order"] }
tempfile = "3.23.0"
thiserror = "2.0.17"
tokio = { version = "1", features = ["full"] }
tracing = "0.1.41"
"#;
