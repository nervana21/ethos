#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Bitcoin Protocol Codegen Backend
//!
//! This backend generates Rust client code from Bitcoin RPC method definitions.

use ir::RpcDef;
use thiserror::Error;

/// Backend result containing generated code
#[derive(Debug)]
pub struct BackendResult {
    /// Generated files (filename, content)
    pub files: Vec<(String, String)>,
    /// Backend metadata
    pub metadata: BackendMetadata,
}

/// Backend metadata
#[derive(Debug)]
pub struct BackendMetadata {
    /// Number of files generated
    pub file_count: usize,
    /// Total lines of code generated
    pub total_lines: usize,
    /// Backend name
    pub backend_name: String,
    /// Generation timestamp
    pub generated_at: String,
}

#[derive(Debug, Error)]
/// Errors raised by the codegen backend.
pub enum BackendError {
    /// Generic error variant carrying a message
    #[error("backend error: {0}")]
    Other(String),
}

/// Result alias for the backend.
pub type Result<T> = std::result::Result<T, BackendError>;

/// Common trait for all backends
pub trait Backend {
    /// Generate code from protocol methods
    fn generate(&self, methods: &[RpcDef]) -> Result<BackendResult>;

    /// Get backend name
    fn name(&self) -> &str;

    /// Get backend description
    fn description(&self) -> &str;
}

/// Codegen backend implementation
pub struct CodegenBackend {
    /// Backend configuration
    pub config: CodegenConfig,
}

/// Codegen backend configuration
#[derive(Debug, Clone)]
pub struct CodegenConfig {
    /// Output directory
    pub output_dir: String,
    /// Generate documentation
    pub generate_docs: bool,
    /// Target Rust edition
    pub rust_edition: String,
}

impl Default for CodegenConfig {
    fn default() -> Self {
        Self {
            output_dir: "generated".to_string(),
            generate_docs: true,
            rust_edition: "2021".to_string(),
        }
    }
}

impl CodegenBackend {
    /// Create a new codegen backend
    pub fn new(config: CodegenConfig) -> Self { Self { config } }
}

impl Backend for CodegenBackend {
    fn generate(&self, methods: &[RpcDef]) -> Result<BackendResult> {
        let mut files = Vec::new();
        let mut total_lines = 0;

        // Generate client trait
        let client_trait = self.generate_client_trait(methods)?;
        files.push(("client_trait.rs".to_string(), client_trait.clone()));
        total_lines += client_trait.lines().count();

        // Generate response types
        let response_types = self.generate_response_types(methods)?;
        files.push(("responses.rs".to_string(), response_types.clone()));
        total_lines += response_types.lines().count();

        // Generate method implementations
        for method in methods {
            let method_impl = self.generate_method_impl(method)?;
            let filename = format!("{}.rs", method.name);
            files.push((filename, method_impl.clone()));
            total_lines += method_impl.lines().count();
        }

        // Generate documentation if enabled
        if self.config.generate_docs {
            let docs = self.generate_documentation(methods)?;
            files.push(("README.md".to_string(), docs.clone()));
            total_lines += docs.lines().count();
        }

        let file_count = files.len();
        Ok(BackendResult {
            files,
            metadata: BackendMetadata {
                file_count,
                total_lines,
                backend_name: self.name().to_string(),
                generated_at: chrono::Utc::now().to_rfc3339(),
            },
        })
    }

    fn name(&self) -> &str { "codegen" }

    fn description(&self) -> &str {
        "Generates Rust client code from Bitcoin RPC method definitions"
    }
}

impl CodegenBackend {
    /// Generate client trait
    fn generate_client_trait(&self, methods: &[RpcDef]) -> Result<String> {
        let mut code = String::new();

        code.push_str("//! Bitcoin RPC Client Trait\n\n");
        code.push_str("use serde_json::Value;\n");
        code.push_str("/// Bitcoin RPC Client Trait\n");
        code.push_str("pub trait BitcoinRpcClient {\n");

        for method in methods {
            let method_sig = self.generate_method_signature(method);
            code.push_str(&format!("    {}\n", method_sig));
        }

        code.push_str("}\n");

        Ok(code)
    }

    /// Generate response types
    fn generate_response_types(&self, methods: &[RpcDef]) -> Result<String> {
        let mut code = String::new();

        code.push_str("//! Bitcoin RPC Response Types\n\n");
        code.push_str("use serde::{Deserialize, Serialize};\n\n");

        for method in methods {
            if method.result.is_some() {
                let response_type = self.generate_response_type(method);
                code.push_str(&response_type);
                code.push('\n');
            }
        }

        Ok(code)
    }

    /// Generate method implementation
    fn generate_method_impl(&self, method: &RpcDef) -> Result<String> {
        let mut code = String::new();

        code.push_str(&format!("//! Implementation for {}\n\n", method.name));
        code.push_str("use serde_json::Value;\n");

        let method_impl = self.generate_method_implementation(method);
        code.push_str(&method_impl);

        Ok(code)
    }

    /// Generate documentation
    fn generate_documentation(&self, methods: &[RpcDef]) -> Result<String> {
        let mut docs = String::new();

        docs.push_str("# Bitcoin RPC Client\n\n");
        docs.push_str("Generated Bitcoin RPC client with the following methods:\n\n");

        for method in methods {
            docs.push_str(&format!("- **{}**: {}\n", method.name, method.description));
        }

        docs.push_str("\n## Usage\n\n");
        docs.push_str("```rust\n");
        docs.push_str("use btc_clients::client::BitcoinRpcClient;\n");
        docs.push_str("// ...\n");
        docs.push_str("```\n");

        Ok(docs)
    }

    /// Generate method signature
    fn generate_method_signature(&self, method: &RpcDef) -> String {
        let params: Vec<String> =
            method.params.iter().map(|p| format!("{}: {}", p.name, p.param_type.name)).collect();

        let param_list =
            if params.is_empty() { "".to_string() } else { format!(", {}", params.join(", ")) };

        let return_type = if let Some(result) = &method.result {
            result.name.clone()
        } else {
            "Value".to_string()
        };
        format!("async fn {}(&self{}) -> Result<{}>;", method.name, param_list, return_type)
    }

    /// Generate response type
    fn generate_response_type(&self, method: &RpcDef) -> String {
        if method.result.is_none()
            || method.result.as_ref().map(|r| r.name.as_str()) == Some("Value")
        {
            return String::new();
        }

        format!(
            "#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct {}Response {{\n    // TODO: Add fields based on return type\n}}\n",
            method.name
        )
    }

    /// Generate method implementation
    fn generate_method_implementation(&self, method: &RpcDef) -> String {
        let return_type = if let Some(result) = &method.result {
            result.name.clone()
        } else {
            "Value".to_string()
        };
        format!(
            "pub async fn {}(&self) -> Result<{}> {{\n    // TODO: Implement method\n    todo!()\n}}",
            method.name,
            return_type
        )
    }
}
