use std::fmt::Write as _;

use ir::RpcDef;

use crate::CodeGenerator;

/// Code generator that creates the transport infrastructure for RPC communication
pub struct TransportInfrastructureGenerator {
    protocol: String,
}

impl TransportInfrastructureGenerator {
    /// Create a new TransportInfrastructureGenerator for the specified protocol
    pub fn new(protocol: impl Into<String>) -> Self { Self { protocol: protocol.into() } }
}

impl CodeGenerator for TransportInfrastructureGenerator {
    fn generate(&self, _methods: &[RpcDef]) -> Vec<(String, String)> {
        let mut code = String::new();

        match self.protocol.as_str() {
            "unix" => {
                emit_unix_socket_imports(&mut code);
                emit_unix_socket_error_enum(&mut code);
                emit_unix_socket_error_impls(&mut code);
                emit_transport_trait(&mut code);
                emit_transport_ext_trait(&mut code);
                emit_transport_ext_impl(&mut code);
                emit_unix_socket_transport_struct(&mut code);
                emit_unix_socket_transport_impl(&mut code);
                emit_unix_socket_transport_trait_impl(&mut code);
            }
            "http" => {
                emit_imports(&mut code);
                emit_error_enum(&mut code);
                emit_error_impls(&mut code);
                emit_transport_trait(&mut code);
                emit_transport_ext_trait(&mut code);
                emit_transport_ext_impl(&mut code);
                emit_default_transport_struct(&mut code);
                emit_default_transport_impl(&mut code);
                emit_transport_impl(&mut code);
            }
            _ => {
                // For unsupported protocols, generate a placeholder with an error message
                code.push_str(&format!(
					"// Error: Unsupported transport protocol: {}. Supported protocols: unix, http\n",
					self.protocol
				));
            }
        }

        vec![("core.rs".to_string(), code)]
    }
}

fn emit_imports(code: &mut String) {
    writeln!(
        code,
        "use serde_json::Value;\n\
     use thiserror::Error;\n\
     use reqwest;\n\
     use serde;\n"
    )
    .expect("Failed to write imports");
}

fn emit_error_enum(code: &mut String) {
    writeln!(
        code,
        "/// Errors that can occur during RPC transport operations\n\
         #[derive(Debug, Error, serde::Serialize, serde::Deserialize)]\n\
         pub enum TransportError {{\n\
             /// HTTP communication error\n\
             #[error(\"HTTP error: {{0}}\")] Http(String),\n\
             /// JSON serialization/deserialization error\n\
             #[error(\"JSON error: {{0}}\")] Json(String),\n\
             /// RPC protocol error\n\
             #[error(\"RPC error: {{0}}\")] Rpc(String),\n\
             /// Network connection error\n\
             #[error(\"Connection error: {{0}}\")] ConnectionError(String),\n\
         }}\n"
    )
    .expect("Failed to write error enum");
}

fn emit_error_impls(code: &mut String) {
    for (from, variant) in
        &[("reqwest::Error", "Http"), ("serde_json::Error", "Json"), ("std::io::Error", "Rpc")]
    {
        writeln!(
            code,
            "impl From<{from}> for TransportError {{\n\
                 fn from(err: {from}) -> Self {{\n\
                     TransportError::{variant}(err.to_string())\n\
                 }}\n\
             }}\n"
        )
        .expect("Failed to write error impl");
    }
}

fn emit_transport_trait(code: &mut String) {
    writeln!(
        code,
        "/// Core trait for RPC transport operations\n\
         pub trait TransportTrait: Send + Sync {{\n\
             /// Send a single RPC request and return the response\n\
             fn send_request<'a>(&'a self, method: &'a str, params: &'a [Value]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, TransportError>> + Send + 'a>>;\n\
             \n\
             /// Send a **batch** of raw JSON-RPC objects in one HTTP call.\n\
             ///\n\
             /// The `bodies` slice is already serializable JSON-RPC-2.0 frames:\n\
             ///   [ {{ \"jsonrpc\":\"2.0\", \"id\":0, \"method\":\"foo\", \"params\": [...] }}, … ]\n\
             fn send_batch<'a>(\n\
                 &'a self,\n\
                 bodies: &'a [Value],\n\
             ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Value>, TransportError>> + Send + 'a>>;\n\
             \n\
             /// Get the URL endpoint for this transport\n\
             fn url(&self) -> &str;\n\
         }}"
    )
    .expect("Failed to write transport trait");
}

fn emit_transport_ext_trait(code: &mut String) {
    writeln!(
        code,
        "/// Extended transport trait with type-safe RPC calls\n\
         pub trait TransportExt {{\n\
             /// Send a type-safe RPC request and deserialize the response\n\
             fn call<'a, T: serde::de::DeserializeOwned>(&'a self, method: &'a str, params: &'a [Value]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, TransportError>> + Send + 'a>>;\n\
         }}\n"
    )
    .expect("Failed to write transport ext trait");
}

fn emit_transport_ext_impl(code: &mut String) {
    writeln!(
        code,
        "impl<T: TransportTrait> TransportExt for T {{\n\
             fn call<'a, T2: serde::de::DeserializeOwned>(&'a self, method: &'a str, params: &'a [Value]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T2, TransportError>> + Send + 'a>> {{\n\
                 Box::pin(async move {{\n\
                     let result = self.send_request(method, params).await?;\n\
                     Ok(serde_json::from_value(result)?)\n\
                 }})\n\
             }}\n\
         }}\n"
    )
    .expect("Failed to write transport ext impl");
}

fn emit_default_transport_struct(code: &mut String) {
    writeln!(
        code,
        "/// Default HTTP transport implementation for RPC communication\n\
         #[derive(Clone, Debug)]\n\
         pub struct DefaultTransport {{\n\
             /// HTTP client for making requests\n\
             client: reqwest::Client,\n\
             /// RPC endpoint URL\n\
             url: String,\n\
             /// Optional authentication credentials (username, password)\n\
             auth: Option<(String, String)>,\n\
             /// Optional wallet name for Bitcoin Core RPC calls\n\
             wallet_name: Option<String>,\n\
         }}\n"
    )
    .expect("Failed to write default transport struct");
}

fn emit_default_transport_impl(code: &mut String) {
    writeln!(
        code,
        "impl DefaultTransport {{\n\
             /// Create a new default transport with the given URL and optional authentication.\n\
             ///\n\
             /// # Arguments\n\
             /// * `url` - The RPC endpoint URL\n\
             /// * `auth` - Optional (username, password) tuple for authentication\n\
             pub fn new(url: impl Into<String>, auth: Option<(String, String)>) -> Self {{\n\
                 Self {{\n\
                     client: reqwest::Client::new(),\n\
                     url: url.into(),\n\
                     auth,\n\
                     wallet_name: None,\n\
                 }}\n\
             }}\n\
             \n\
             /// Configure this transport to use a specific wallet for RPC calls.\n\
             ///\n\
             /// # Arguments\n\
             /// * `wallet_name` - The name of the wallet to use for RPC calls\n\
             pub fn with_wallet(mut self, wallet_name: impl Into<String>) -> Self {{\n\
                 self.wallet_name = Some(wallet_name.into());\n\
                 self\n\
             }}\n\
         }}\n"
    )
    .expect("Failed to write default transport impl");
}

fn emit_transport_impl(code: &mut String) {
    writeln!(
        code,
        "impl TransportTrait for DefaultTransport {{
    fn send_request<'a>(&'a self, method: &'a str, params: &'a [Value]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, TransportError>> + Send + 'a>> {{
        let client = self.client.clone();
        let url = self.url.clone();
        let auth = self.auth.clone();
        let wallet_name = self.wallet_name.clone();
        Box::pin(async move {{
            let request = serde_json::json!({{
                \"jsonrpc\": \"2.0\", \"id\": \"1\", \"method\": method, \"params\": params
            }});

            // If a wallet is configured, prefer wallet endpoint; fallback to base URL on -32601 (method not found)
            if let Some(wallet) = &wallet_name {{
                let wallet_url = format!(\"{{}}/wallet/{{}}\", url.trim_end_matches('/'), wallet);

                // Try wallet endpoint first
                let mut req = client.post(&wallet_url).json(&request);
                if let Some((username, password)) = &auth {{
                    req = req.basic_auth(username, Some(password));
                }}
                let response = match req.send().await {{
                    Ok(resp) => resp,
                    Err(e) => return Err(TransportError::Http(e.to_string())),
                }};

                let text = response.text().await.map_err(|e| TransportError::Http(e.to_string()))?;
                let json: Value = serde_json::from_str(&text).map_err(|e| TransportError::Json(e.to_string()))?;

                if let Some(error) = json.get(\"error\") {{
                    // Fallback only for -32601 (Method not found)
                    if error.get(\"code\").and_then(|c| c.as_i64()) == Some(-32601) {{
                        let mut req = client.post(&url).json(&request);
                        if let Some((username, password)) = &auth {{
                            req = req.basic_auth(username, Some(password));
                        }}
                        let response = match req.send().await {{
                            Ok(resp) => resp,
                            Err(e) => return Err(TransportError::Http(e.to_string())),
                        }};
                        let text = response.text().await.map_err(|e| TransportError::Http(e.to_string()))?;
                        let json: Value = serde_json::from_str(&text).map_err(|e| TransportError::Json(e.to_string()))?;
                        if let Some(error) = json.get(\"error\") {{
                            return Err(TransportError::Rpc(error.to_string()));
                        }}
                        return json.get(\"result\").cloned().ok_or_else(|| TransportError::Rpc(\"No result field\".to_string()));
                    }} else {{
                        return Err(TransportError::Rpc(error.to_string()));
                    }}
                }}

                return json.get(\"result\").cloned().ok_or_else(|| TransportError::Rpc(\"No result field\".to_string()));
            }}

            // No wallet configured → base URL
            let mut req = client.post(&url).json(&request);
            if let Some((username, password)) = &auth {{
                req = req.basic_auth(username, Some(password));
            }}
            let response = match req.send().await {{
                Ok(resp) => resp,
                Err(e) => return Err(TransportError::Http(e.to_string())),
            }};
            let text = response.text().await.map_err(|e| TransportError::Http(e.to_string()))?;
            let json: Value = serde_json::from_str(&text).map_err(|e| TransportError::Json(e.to_string()))?;
            if let Some(error) = json.get(\"error\") {{
                return Err(TransportError::Rpc(error.to_string()));
            }}
            json.get(\"result\").cloned().ok_or_else(|| TransportError::Rpc(\"No result field\".to_string()))
        }})
    }}

    fn send_batch<'a>(
        &'a self,
        bodies: &'a [Value],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Value>, TransportError>> + Send + 'a>> {{
        let client = self.client.clone();
        let url = self.url.clone();
        let auth = self.auth.clone();
        Box::pin(async move {{
            let mut req = client.post(&url).json(bodies);
            if let Some((username, password)) = &auth {{
                req = req.basic_auth(username, Some(password));
            }}
            let response = match req.send().await {{
                Ok(resp) => resp,
                Err(e) => return Err(TransportError::Http(e.to_string())),
            }};
            let text = response.text().await.map_err(|e| TransportError::Http(e.to_string()))?;
            let v: Vec<Value> = serde_json::from_str(&text).map_err(|e| TransportError::Json(e.to_string()))?;
            Ok(v)
        }})
    }}

    fn url(&self) -> &str {{
        &self.url
    }}
}}"
    )
    .expect("Failed to write transport impl");
}

// Unix socket transport functions for Core Lightning
fn emit_unix_socket_imports(code: &mut String) {
    writeln!(
        code,
        "use serde_json::Value;\n\
         use thiserror::Error;\n\
         use std::path::PathBuf;\n\
         use tokio::net::UnixStream;\n\
         use tokio::io::{{AsyncWriteExt, AsyncReadExt}};\n\
         use serde;\n"
    )
    .expect("Failed to write unix socket imports");
}

fn emit_unix_socket_error_enum(code: &mut String) {
    writeln!(
        code,
        "/// Errors that can occur during Unix socket RPC transport operations\n\
         #[derive(Debug, Error, serde::Serialize, serde::Deserialize)]\n\
         pub enum TransportError {{\n\
             /// Unix socket communication error\n\
             #[error(\"Unix socket error: {{0}}\")] UnixSocket(String),\n\
             /// JSON serialization/deserialization error\n\
             #[error(\"JSON error: {{0}}\")] Json(String),\n\
             /// RPC protocol error\n\
             #[error(\"RPC error: {{0}}\")] Rpc(String),\n\
             /// Network connection error\n\
             #[error(\"Connection error: {{0}}\")] ConnectionError(String),\n\
         }}\n"
    )
    .expect("Failed to write unix socket error enum");
}

fn emit_unix_socket_error_impls(code: &mut String) {
    for (from, variant) in &[("std::io::Error", "UnixSocket"), ("serde_json::Error", "Json")] {
        writeln!(
            code,
            "impl From<{from}> for TransportError {{\n\
                 fn from(err: {from}) -> Self {{\n\
                     TransportError::{variant}(err.to_string())\n\
                 }}\n\
             }}\n"
        )
        .expect("Failed to write unix socket error impl");
    }
}

fn emit_unix_socket_transport_struct(code: &mut String) {
    writeln!(
        code,
        "/// Default Unix socket transport implementation for Core Lightning RPC\n\
         #[derive(Clone, Debug)]\n\
         pub struct DefaultTransport {{\n\
             /// Path to the Unix socket file\n\
             socket_path: PathBuf,\n\
         }}\n"
    )
    .expect("Failed to write unix socket transport struct");
}

fn emit_unix_socket_transport_impl(code: &mut String) {
    writeln!(
        code,
        "impl DefaultTransport {{\n\
             pub fn new(socket_path: impl Into<PathBuf>) -> Self {{\n\
                 Self {{\n\
                     socket_path: socket_path.into(),\n\
                 }}\n\
             }}\n\
         }}\n"
    )
    .expect("Failed to write unix socket transport impl");
}

fn emit_unix_socket_transport_trait_impl(code: &mut String) {
    writeln!(
        code,
        "impl TransportTrait for DefaultTransport {{
    fn send_request<'a>(&'a self, method: &'a str, params: &'a [Value]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, TransportError>> + Send + 'a>> {{
        let socket_path = self.socket_path.clone();
        Box::pin(async move {{
            let request = serde_json::json!({{
                \"jsonrpc\": \"2.0\", \"id\": \"1\", \"method\": method, \"params\": params
            }});

            let mut stream = UnixStream::connect(&socket_path).await
                .map_err(|e| TransportError::ConnectionError(format!(\"Failed to connect to socket {{:?}}: {{}}\", socket_path, e)))?;

            let request_str = serde_json::to_string(&request)
                .map_err(|e| TransportError::Json(e.to_string()))?;

            stream.write_all(request_str.as_bytes()).await
                .map_err(|e| TransportError::UnixSocket(e.to_string()))?;
            stream.write_all(b\"\\n\").await
                .map_err(|e| TransportError::UnixSocket(e.to_string()))?;

            // Read response byte by byte until we get a complete JSON response
            let mut response = Vec::new();
            let mut buffer = [0; 1];

            loop {{
                match stream.read(&mut buffer).await {{
                    Ok(0) => {{
                        return Err(TransportError::UnixSocket(\"Connection closed by server\".to_string()));
                    }}
                    Ok(n) => {{
                        response.extend_from_slice(&buffer[..n]);
                        let response_str = String::from_utf8_lossy(&response);
                        // Check if we have a complete JSON response
                        if let Ok(_parsed) = serde_json::from_str::<serde_json::Value>(&response_str) {{
                            // We have a complete JSON response, break out of the loop
                            break;
                        }}
                    }}
                    Err(e) => {{
                        return Err(TransportError::UnixSocket(e.to_string()));
                    }}
                }}
            }}

            let response = String::from_utf8_lossy(&response).to_string();
            let json: Value = serde_json::from_str(&response)
                .map_err(|e| TransportError::Json(e.to_string()))?;

            if let Some(error) = json.get(\"error\") {{
                return Err(TransportError::Rpc(error.to_string()));
            }}
            json.get(\"result\").cloned().ok_or_else(|| TransportError::Rpc(\"No result field\".to_string()))
        }})
    }}

    fn send_batch<'a>(
        &'a self,
        bodies: &'a [Value],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Value>, TransportError>> + Send + 'a>> {{
        let socket_path = self.socket_path.clone();
        Box::pin(async move {{
            let mut stream = UnixStream::connect(&socket_path).await
                .map_err(|e| TransportError::ConnectionError(format!(\"Failed to connect to socket {{:?}}: {{}}\", socket_path, e)))?;

            let request_str = serde_json::to_string(bodies)
                .map_err(|e| TransportError::Json(e.to_string()))?;

            stream.write_all(request_str.as_bytes()).await
                .map_err(|e| TransportError::UnixSocket(e.to_string()))?;
            stream.write_all(b\"\\n\").await
                .map_err(|e| TransportError::UnixSocket(e.to_string()))?;

            // Read response byte by byte until we get a complete JSON response
            let mut response = Vec::new();
            let mut buffer = [0; 1];

            loop {{
                match stream.read(&mut buffer).await {{
                    Ok(0) => {{
                        return Err(TransportError::UnixSocket(\"Connection closed by server\".to_string()));
                    }}
                    Ok(n) => {{
                        response.extend_from_slice(&buffer[..n]);
                        let response_str = String::from_utf8_lossy(&response);
                        // Check if we have a complete JSON response
                        if let Ok(_parsed) = serde_json::from_str::<serde_json::Value>(&response_str) {{
                            // We have a complete JSON response, break out of the loop
                            break;
                        }}
                    }}
                    Err(e) => {{
                        return Err(TransportError::UnixSocket(e.to_string()));
                    }}
                }}
                // Lightweight idle timeout to avoid infinite hang on partial frames
            }}

            let response = String::from_utf8_lossy(&response).to_string();
            let v: Vec<Value> = serde_json::from_str(&response)
                .map_err(|e| TransportError::Json(e.to_string()))?;
            Ok(v)
        }})
    }}

    fn url(&self) -> &str {{
        // For Unix sockets, we return the socket path as a string
        self.socket_path
            .to_str()
            .expect(\"valid socket path required; set CLN_RPC_SOCKET or CLN_LIGHTNING_DIR\")
    }}
}}"
    )
    .expect("Failed to write unix socket transport trait impl");
}
