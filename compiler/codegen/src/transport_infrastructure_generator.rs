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
        "use std::time::Duration;\n\
\n\
use base64::{{engine::general_purpose, Engine}};\n\
use bitreq::{{post, Client as BitreqClient, Error as BitreqError, RequestExt}};\n\
use serde;\n\
use serde_json::Value;\n\
use thiserror::Error;\n\
use tokio::time::sleep;\n\
use tracing::warn;\n"
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
             /// JSON serialization error (request)\n\
             #[error(\"JSON error: {{0}}\")] Json(String),\n\
             /// RPC protocol error\n\
             #[error(\"RPC error: {{0}}\")] Rpc(String),\n\
             /// Network connection error\n\
             #[error(\"Connection error: {{0}}\")] ConnectionError(String),\n\
             /// Redirect error, not retryable\n\
             #[error(\"HttpRedirect: {{0}}\")] HttpRedirect(String),\n\
             /// Error decoding the response\n\
             #[error(\"Malformed Response: {{0}}\")] MalformedResponse(String),\n\
             /// Error parsing RPC response\n\
             #[error(\"Error parsing rpc response: {{0}}\")] Parse(String),\n\
             /// Maximum retries exceeded\n\
             #[error(\"Max retries {{0}} exceeded\")] MaxRetriesExceeded(u8),\n\
         }}\n"
    )
    .expect("Failed to write error enum");
}

fn emit_error_impls(code: &mut String) {
    writeln!(
        code,
        "impl From<BitreqError> for TransportError {{
    fn from(value: BitreqError) -> Self {{
        match value {{
            // Connection errors
            BitreqError::AddressNotFound
            | BitreqError::IoError(_)
            | BitreqError::RustlsCreateConnection(_) => TransportError::ConnectionError(value.to_string()),

            // Redirect errors
            BitreqError::RedirectLocationMissing
            | BitreqError::InfiniteRedirectionLoop
            | BitreqError::TooManyRedirections => TransportError::HttpRedirect(value.to_string()),

            // Size/parsing errors
            BitreqError::HeadersOverflow
            | BitreqError::StatusLineOverflow
            | BitreqError::BodyOverflow
            | BitreqError::MalformedChunkLength
            | BitreqError::MalformedChunkEnd
            | BitreqError::MalformedContentLength
            | BitreqError::InvalidUtf8InResponse
            | BitreqError::InvalidUtf8InBody(_) => {{
                TransportError::MalformedResponse(value.to_string())
            }}

            // Other errors
            _ => TransportError::Http(value.to_string()),
        }}
    }}
}}

impl From<serde_json::Error> for TransportError {{
    fn from(err: serde_json::Error) -> Self {{
        TransportError::Json(err.to_string())
    }}
}}

impl From<std::io::Error> for TransportError {{
    fn from(err: std::io::Error) -> Self {{
        TransportError::Rpc(err.to_string())
    }}
}}
"
    )
    .expect("Failed to write error impl");
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
         #[derive(Clone)]\n\
         pub struct DefaultTransport {{\n\
             /// HTTP client for making requests (reuses TCP connections)\n\
             client: BitreqClient,\n\
             /// RPC endpoint URL\n\
             url: String,\n\
             /// Precomputed Basic auth header value, or None\n\
             authorization: Option<String>,\n\
             /// Timeout for requests in seconds\n\
             timeout_secs: u64,\n\
             /// Maximum number of retries per request\n\
             max_retries: u8,\n\
             /// Interval between retries in ms\n\
             retry_interval: u64,\n\
             /// Optional wallet name for Bitcoin Core RPC calls\n\
             wallet_name: Option<String>,\n\
         }}\n"
    )
    .expect("Failed to write default transport struct");
}

fn emit_default_transport_impl(code: &mut String) {
    writeln!(
        code,
        "/// The default capacity for the HTTP client connection pool.\n\
         const DEFAULT_HTTP_CLIENT_CAPACITY: usize = 10;\n\
         /// Timeout for a request in seconds.\n\
         const DEFAULT_TIMEOUT_SECONDS: u64 = 30;\n\
         /// Maximum number of retries for a request.\n\
         const DEFAULT_MAX_RETRIES: u8 = 3;\n\
         /// Interval between retries in ms.\n\
         const DEFAULT_RETRY_INTERVAL_MS: u64 = 1_000;\n\
         \n\
         impl DefaultTransport {{\n\
             /// Create a new default transport with the given URL and optional authentication.\n\
             ///\n\
             /// # Arguments\n\
             /// * `url` - The RPC endpoint URL\n\
             /// * `auth` - Optional (username, password) tuple for authentication\n\
             pub fn new(url: impl Into<String>, auth: Option<(String, String)>) -> Self {{\n\
                 let authorization = auth.as_ref().map(|(u, p)| {{\n\
                     format!(\"Basic {{}}\", general_purpose::STANDARD.encode(format!(\"{{}}:{{}}\", u, p)))\n\
                 }});\n\
                 Self {{\n\
                     client: BitreqClient::new(DEFAULT_HTTP_CLIENT_CAPACITY),\n\
                     url: url.into(),\n\
                     authorization,\n\
                     timeout_secs: DEFAULT_TIMEOUT_SECONDS,\n\
                     max_retries: DEFAULT_MAX_RETRIES,\n\
                     retry_interval: DEFAULT_RETRY_INTERVAL_MS,\n\
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
             \n\
             /// Returns `true` if the error is potentially recoverable and should be retried.\n\
             fn is_bitreq_error_recoverable(err: &BitreqError) -> bool {{\n\
                 match err {{\n\
                     // Connection/network errors - might be recoverable\n\
                     BitreqError::AddressNotFound\n\
                     | BitreqError::IoError(_)\n\
                     | BitreqError::RustlsCreateConnection(_) => {{\n\
                         warn!(err = %err, \"connection error, retrying...\");\n\
                         true\n\
                     }}\n\
                     \n\
                     // Redirect errors - not retryable\n\
                     BitreqError::RedirectLocationMissing => false,\n\
                     BitreqError::InfiniteRedirectionLoop => false,\n\
                     BitreqError::TooManyRedirections => false,\n\
                     \n\
                     // Size limit errors - not retryable\n\
                     BitreqError::HeadersOverflow => false,\n\
                     BitreqError::StatusLineOverflow => false,\n\
                     BitreqError::BodyOverflow => false,\n\
                     \n\
                     // Protocol/parsing errors - might be recoverable\n\
                     BitreqError::MalformedChunkLength\n\
                     | BitreqError::MalformedChunkEnd\n\
                     | BitreqError::MalformedContentLength\n\
                     | BitreqError::InvalidUtf8InResponse => {{\n\
                         warn!(err = %err, \"malformed response, retrying...\");\n\
                         true\n\
                     }}\n\
                     \n\
                     // UTF-8 in body - not retryable\n\
                     BitreqError::InvalidUtf8InBody(_) => false,\n\
                     \n\
                     // HTTPS not enabled - not retryable\n\
                     BitreqError::HttpsFeatureNotEnabled => false,\n\
                     \n\
                     // Other errors - not retryable\n\
                     BitreqError::Other(_) => false,\n\
                     \n\
                     // Non-exhaustive match fallback\n\
                     _ => false,\n\
                 }}\n\
             }}\n\
         }}\n\
         \n\
         impl std::fmt::Debug for DefaultTransport {{\n\
             fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n\
                 f.debug_struct(\"DefaultTransport\")\n\
                     .field(\"url\", &self.url)\n\
                     .field(\"timeout_secs\", &self.timeout_secs)\n\
                     .field(\"max_retries\", &self.max_retries)\n\
                     .field(\"retry_interval\", &self.retry_interval)\n\
                     .field(\"wallet_name\", &self.wallet_name)\n\
                     .finish_non_exhaustive()\n\
             }}\n\
         }}\n"
    )
    .expect("Failed to write default transport impl");
}

fn emit_transport_impl(code: &mut String) {
    writeln!(
        code,
        "/// Internal error type for `do_request` to distinguish network errors from other transport errors.
/// This allows the retry logic to check recoverability on raw `BitreqError` variants.
enum DoRequestError {{
    /// Network error from bitreq - check recoverability before converting
    Network(BitreqError),
    /// Other transport error - not recoverable via network retry
    Transport(TransportError),
}}

impl TransportTrait for DefaultTransport {{
    fn send_request<'a>(&'a self, method: &'a str, params: &'a [Value]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, TransportError>> + Send + 'a>> {{
        let client = self.client.clone();
        let url = self.url.clone();
        let authorization = self.authorization.clone();
        let wallet_name = self.wallet_name.clone();
        let timeout_secs = self.timeout_secs;
        let max_retries = self.max_retries;
        let retry_interval = self.retry_interval;

        async fn do_request(
            client: &BitreqClient,
            url: &str,
            authorization: &Option<String>,
            request: &serde_json::Value,
            timeout_secs: u64,
        ) -> Result<Value, DoRequestError> {{
            let body = serde_json::to_vec(request).map_err(|e| DoRequestError::Transport(TransportError::Json(e.to_string())))?;
            let mut req = post(url)
                .with_header(\"Content-Type\", \"application/json\")
                .with_body(body)
                .with_timeout(timeout_secs);
            if let Some(ref h) = authorization {{ req = req.with_header(\"Authorization\", h); }}
            let response = req.send_async_with_client(client).await.map_err(DoRequestError::Network)?;
            let status_code = response.status_code;
            if !(200..300).contains(&status_code) {{
                return Err(DoRequestError::Transport(TransportError::Http(format!(\"{{}} {{}}\", status_code, response.reason_phrase))));
            }}
            let raw = response.as_str().map_err(|e: BitreqError| DoRequestError::Transport(TransportError::Parse(e.to_string())))?;
            let json: Value = serde_json::from_str(raw).map_err(|e| DoRequestError::Transport(TransportError::Parse(e.to_string())))?;
            if let Some(error) = json.get(\"error\") {{
                if !error.is_null() {{
                    return Err(DoRequestError::Transport(TransportError::Rpc(error.to_string())));
                }}
            }}
            json.get(\"result\").cloned().ok_or_else(|| DoRequestError::Transport(TransportError::Rpc(\"No result field\".to_string())))
        }}

        Box::pin(async move {{
            let request = serde_json::json!({{
                \"jsonrpc\": \"2.0\", \"id\": \"1\", \"method\": method, \"params\": params
            }});
            let mut retries = 0u8;
            loop {{
                let target_url = if let Some(ref wallet) = wallet_name {{
                    format!(\"{{}}/wallet/{{}}\", url.trim_end_matches('/'), wallet)
                }} else {{
                    url.clone()
                }};
                match do_request(&client, &target_url, &authorization, &request, timeout_secs).await {{
                    Ok(v) => return Ok(v),
                    Err(DoRequestError::Transport(TransportError::Rpc(ref msg))) if wallet_name.is_some() && msg.contains(\"\\\"code\\\":-32601\") => {{
                        match do_request(&client, &url, &authorization, &request, timeout_secs).await {{
                            Ok(v) => return Ok(v),
                            Err(DoRequestError::Network(e)) => return Err(TransportError::from(e)),
                            Err(DoRequestError::Transport(e)) => return Err(e),
                        }}
                    }}
                    Err(DoRequestError::Network(bitreq_err)) => {{
                        if !Self::is_bitreq_error_recoverable(&bitreq_err) {{
                            return Err(TransportError::from(bitreq_err));
                        }}
                        // Error is recoverable, will retry after incrementing counter
                    }}
                    Err(DoRequestError::Transport(err)) => {{
                        // Non-network transport errors are not recoverable
                        return Err(err);
                    }}
                }}
                retries += 1;
                if retries >= max_retries {{
                    return Err(TransportError::MaxRetriesExceeded(max_retries));
                }}
                sleep(Duration::from_millis(retry_interval)).await;
            }}
        }})
    }}

    // Note: Batch requests do not retry on failure. Callers should implement
    // their own retry logic if needed.
    fn send_batch<'a>(
        &'a self,
        bodies: &'a [Value],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Value>, TransportError>> + Send + 'a>> {{
        let client = self.client.clone();
        let url = self.url.clone();
        let authorization = self.authorization.clone();
        let timeout_secs = self.timeout_secs;
        Box::pin(async move {{
            let bodies_vec: Vec<Value> = bodies.to_vec();
            let body = serde_json::to_vec(&bodies_vec).map_err(|e| TransportError::Json(e.to_string()))?;
            let mut req = post(&url)
                .with_header(\"Content-Type\", \"application/json\")
                .with_body(body)
                .with_timeout(timeout_secs);
            if let Some(ref h) = authorization {{ req = req.with_header(\"Authorization\", h); }}
            let response = req.send_async_with_client(&client).await?;
            let status_code = response.status_code;
            if !(200..300).contains(&status_code) {{
                return Err(TransportError::Http(format!(\"HTTP {{}}\", status_code)));
            }}
            let raw = response.as_str().map_err(|e: BitreqError| TransportError::Parse(e.to_string()))?;
            let v: Vec<Value> = serde_json::from_str(raw).map_err(|e| TransportError::Parse(e.to_string()))?;
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

// Unix socket transport functions (for implementations using Unix socket RPC)
fn emit_unix_socket_imports(code: &mut String) {
    writeln!(
        code,
        "use std::path::PathBuf;\n\
\n\
use serde;\n\
use serde_json::Value;\n\
use thiserror::Error;\n\
use tokio::io::{{AsyncReadExt, AsyncWriteExt}};\n\
use tokio::net::UnixStream;\n"
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
                if !error.is_null() {{
                    return Err(TransportError::Rpc(error.to_string()));
                }}
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
