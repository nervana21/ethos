#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Code generation utilities for Bitcoin Core JSON-RPC.
//!
//! This crate turns `Method` descriptors into ready-to`cargo check` Rust modules.
//! It focuses solely on code generation: parsing API metadata, scaffolding module hierarchies,
//! generating transport-layer clients, strongly-typed response structs, and test-node helpers.
//!
//! Other responsibilities—such as runtime testing, node spawning, or API discovery logic—reside in companion crates.

pub mod generators;

use std::fs::{self};
use std::path::Path;
use std::process::Command;

use ir::{ProtocolIR, RpcDef, TypeDef, TypeKind};
use registry::TypeAliasRegistry;
use thiserror::Error;
use types::ProtocolVersion;

use crate::generators::doc_comment;

/// Error type for code generation operations in this crate.
#[derive(Debug, Error)]
pub enum CodegenError {
    /// Underlying I/O error while reading or writing files.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization/deserialization error.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Formatting error when building generated source.
    #[error(transparent)]
    Fmt(#[from] std::fmt::Error),
    /// Generic message-based error.
    #[error("{0}")]
    Message(String),
}

impl From<String> for CodegenError {
    fn from(msg: String) -> Self { CodegenError::Message(msg) }
}

/// Convenient result type for codegen functions in this crate.
pub type Result<T> = std::result::Result<T, CodegenError>;

/// Generate Rust type from TypeDef based on TypeKind
pub fn render_type_from_ir(type_def: &TypeDef) -> String {
    match &type_def.kind {
        TypeKind::Primitive => render_primitive(type_def),
        TypeKind::Object => render_struct(type_def),
        TypeKind::Enum => render_enum(type_def),
        TypeKind::Array => render_array(type_def),
        TypeKind::Custom => {
            // Simply refer to the base_type
            type_def.base_type.as_deref().unwrap_or(&type_def.name).to_string()
        }
        TypeKind::Optional => {
            // Handle optional types
            if let Some(base_type) = &type_def.base_type {
                format!("Option<{}>", base_type)
            } else {
                "Option<serde_json::Value>".to_string()
            }
        }
        TypeKind::Alias => {
            // Use base_type if available, otherwise use name
            type_def.base_type.as_deref().unwrap_or(&type_def.name).to_string()
        }
    }
}

/// Render primitive type
fn render_primitive(type_def: &TypeDef) -> String { type_def.name.clone() }

/// Render struct type
fn render_struct(type_def: &TypeDef) -> String {
    // For now, return the name - in a full implementation, this would generate struct code
    type_def.name.clone()
}

/// Render enum type
fn render_enum(type_def: &TypeDef) -> String {
    // For now, return the name - in a full implementation, this would generate enum code
    type_def.name.clone()
}

/// Render array type
fn render_array(type_def: &TypeDef) -> String {
    // For now, return the name - in a full implementation, this would generate array code
    type_def.name.clone()
}

/// Build a protocol registry from ProtocolIR
/// This creates a registry from the IR-based approach using RpcDef objects.
pub fn build_registry_from_ir(protocol_ir: &ProtocolIR) -> ProtocolRegistry {
    let mut reg = ProtocolRegistry::new();

    // Extract RPC methods from the ProtocolIR
    for module in protocol_ir.modules() {
        for definition in module.definitions() {
            if let ir::ProtocolDef::RpcMethod(rpc_def) = definition {
                reg.insert(rpc_def.clone());
            }
        }
    }

    reg
}

/// Re-export the ProtocolRegistry for external use
pub use registry::ProtocolRegistry;

/// Canonical Type Resolver - provides unified type resolution using TypeAliasRegistry
///
/// This struct acts as a lightweight canonical resolution layer that ensures all
/// generated code and proofs use canonical type names. It's the equivalent of
/// `rustc_middle::ty::TyCtxt` for our type system.
#[derive(Debug, Clone)]
pub struct CanonicalTypeResolver<'a> {
    registry: &'a TypeAliasRegistry,
}

impl<'a> CanonicalTypeResolver<'a> {
    /// Create a new CanonicalTypeResolver with the given TypeAliasRegistry
    pub fn new(registry: &'a TypeAliasRegistry) -> Self { Self { registry } }

    /// Resolve a type definition's name to its canonical name
    ///
    /// If the TypeDef has a canonical_name field set, it uses that.
    /// Otherwise, it resolves the type name through the TypeAliasRegistry.
    ///
    /// # Arguments
    /// * `ty` - The TypeDef to resolve
    ///
    /// # Returns
    /// The canonical type name as a String
    pub fn resolve_type_name(&self, ty: &TypeDef) -> String {
        if let Some(canonical) = &ty.canonical_name {
            canonical.clone()
        } else {
            self.registry.resolve(&ty.name).to_string()
        }
    }

    /// Resolve a plain type string to its canonical form
    ///
    /// # Arguments
    /// * `type_name` - The type name to resolve
    ///
    /// # Returns
    /// The canonical type name as a String
    pub fn resolve_str(&self, type_name: &str) -> String {
        self.registry.resolve(type_name).to_string()
    }
}

/// Sub-crate: **`namespace_scaffolder`**
///
/// Writes `mod.rs` scaffolding for generated modules.
/// Given schema versions (`latest`, etc.), it creates:
///
/// - `generated/client/{versions}`
/// - `generated/responses/{versions}`
///
/// plus a top-level `mod.rs` that re-exports everything, so downstream crates can simply
/// use `generated::client::*`.
pub mod namespace_scaffolder;

/// Sub-crate: **`transport_infrastructure_generator`**
///
/// Generates the transport infrastructure types: Transport trait, TransportError enum,
/// and DefaultTransport implementation.
pub mod transport_infrastructure_generator;
pub use generators::NodeManagerGenerator;
pub use transport_infrastructure_generator::TransportInfrastructureGenerator;

/// Sub-crate: **`utils`**
///
/// Utility functions for code generation.
pub mod utils;

/// Defines the core interface for generating Rust source files from a collection of
/// RPC method definitions from the canonical IR. Implementors produce a set of `(filename, source)`
/// pairs and may optionally perform post-generation validation.
///
/// This trait works with `ir::RpcDef` which contains rich semantic information from
/// the analysis phase, including type definitions, categories, and security classifications.
pub trait CodeGenerator {
    /// Generate Rust source files for the provided RPC method definitions.
    fn generate(&self, methods: &[RpcDef]) -> Vec<(String, String)>;

    /// Optional validation step after generation (default is no-op).
    fn validate(&self, _methods: &[RpcDef]) -> Result<()> { Ok(()) }
}

/// Formats a Rust source file using rustfmt with the project's .rustfmt.toml
pub fn format_with_rustfmt(path: &Path) {
    // Find the project root to use its .rustfmt.toml for consistent formatting
    let config_path = path::find_project_root()
        .map(|root| root.join(".rustfmt.toml"))
        .ok()
        .filter(|p| p.exists());

    let mut cmd = Command::new("rustfmt");
    cmd.arg("--edition=2021");
    if let Some(config) = config_path {
        cmd.arg("--config-path").arg(config);
    }
    cmd.arg(path);

    if let Ok(status) = cmd.status() {
        if !status.success() {}
    }
}

/// Trim trailing whitespace from each line and drop trailing blank lines.
/// Always ensures the returned string ends with a single newline when not empty.
fn clean_generated_source(src: &str) -> String {
    let mut lines: Vec<String> = src.lines().map(|l| l.trim_end().to_string()).collect();

    while matches!(lines.last(), Some(line) if line.is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

/// Persist a list of generated source files to disk under the given output directory,
/// creating any necessary subdirectories and appending `.rs` if missing.
pub fn write_generated<P: AsRef<Path>>(
    out_dir: P,
    files: &[(String, String)],
) -> std::io::Result<()> {
    fs::create_dir_all(&out_dir)?;
    for (name, src) in files {
        let path = if name.ends_with(".rs") {
            out_dir.as_ref().join(name)
        } else {
            out_dir.as_ref().join(format!("{name}.rs"))
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let cleaned = clean_generated_source(src);
        fs::write(&path, cleaned.as_bytes())?;
        format_with_rustfmt(&path);
    }
    Ok(())
}

/// Emits async JSON-RPC transport wrappers for Bitcoin Core RPC methods.
///
/// `MethodWrapperGenerator` implements the `CodeGenerator` trait to produce, for each
/// `Method`, a self-contained Rust source file containing:
/// 1. An `async fn` that accepts a `&dyn TransportTrait` and JSON-serializable parameters.
/// 2. Logic to serialize those parameters into a `Vec<serde_json::Value>`.
/// 3. A call to `transport.send_request(method_name, &params).await`.
/// 4. Deserialization of the raw response into a typed `Response` struct (or raw `Value`).
pub struct MethodWrapperGenerator {
    protocol: String, // e.g., "bitcoin_core"
}

impl MethodWrapperGenerator {
    /// Create a new MethodWrapperGenerator with the specified context
    pub fn new(protocol: String) -> Self { Self { protocol } }
}

impl CodeGenerator for MethodWrapperGenerator {
    fn generate(&self, methods: &[RpcDef]) -> Vec<(String, String)> {
        use semantics::method_categorization::group_methods_by_category;

        // Group methods by category
        let categorized_methods = group_methods_by_category(methods);
        let mut files = Vec::new();

        // Generate a file for each category
        for (category, methods_in_category) in categorized_methods {
            let category_name = category.dir_name();
            let mut out = String::from(&format!("//! {} RPC method wrappers\n", category_name));
            out.push_str("//! \n");
            out.push_str(&format!(
                "//! This module contains transport wrappers for {} methods.\n\n",
                category_name
            ));

            // Add imports
            let category_uses_params = methods_in_category.iter().any(|m| !m.params.is_empty());
            if category_uses_params {
                out.push_str("use serde_json::{Value, json};\n");
            } else {
                out.push_str("use serde_json::Value;\n");
            }
            out.push_str("use crate::transport::core::{TransportTrait, TransportError};\n\n");

            // Generate method wrappers for this category
            for m in &methods_in_category {
                /* ---------- fn signature ---------- */
                let fn_args = std::iter::once("transport: &dyn TransportTrait".into())
                    .chain(m.params.iter().map(|param| {
                        let name = crate::utils::sanitize_external_identifier(&param.name);
                        format!("{name}: serde_json::Value")
                    }))
                    .collect::<Vec<_>>()
                    .join(", ");

                /* ---------- params vec ---------- */
                let params_vec = if m.params.is_empty() {
                    "Vec::<Value>::new()".into()
                } else {
                    let elems = m
                        .params
                        .iter()
                        .map(|param| {
                            let name = crate::utils::sanitize_external_identifier(&param.name);
                            format!("json!({name})")
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("vec![{elems}]")
                };

                /* ---------- docs + types ---------- */
                let docs_md = doc_comment::generate_example_docs(m).trim_end().to_string();

                // For transport code generation, we use generic Value types
                let ok_ty = "Value".to_string();

                // Add clippy allow for too many arguments if needed
                let clippy_allow =
                    if m.params.len() > 7 { "#[allow(clippy::too_many_arguments)]\n" } else { "" };

                let fn_name =
                    crate::utils::protocol_rpc_method_to_rust_name(&self.protocol, &m.name)
                        .unwrap_or_else(|e| panic!("{}", e));

                let method_wrapper = format!(
                    r#"{docs}
/// Calls the `{rpc}` RPC method.
///
/// Generated transport wrapper for JSON-RPC.
{clippy_allow}pub async fn {fn_name}({fn_args}) -> Result<{ok_ty}, TransportError> {{
    let params = {params_vec};
    let raw = transport.send_request("{rpc}", &params).await?;
    Ok(raw)
}}

"#,
                    docs = docs_md,
                    rpc = m.name,
                    fn_name = fn_name,
                    fn_args = fn_args,
                    ok_ty = ok_ty,
                    params_vec = params_vec,
                );

                out.push_str(&method_wrapper);
            }

            // Only create file if it has content
            if !methods_in_category.is_empty() {
                files.push((format!("{}.rs", category_name), out));
            }
        }

        files
    }
}

// TODO(multiprocess): Introduce an `RpcComponent` abstraction to formally distinguish between
// independently-addressable RPC components like `node`, `wallet`, `index`, and `gui`.
//
// This will support:
// - routing method calls to different endpoints (e.g., node.sock vs wallet.sock)
// - preventing runtime errors by associating methods with their component
// - future `CombinedClient` that multiplexes requests across components
//
// This abstraction will become essential as Bitcoin Core moves toward
// separate processes with their own RPC servers.
//
// Start by creating a `components.rs` module defining `RpcComponent` and a registry of methods.
