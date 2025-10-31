#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! # `ethos-http` — HTTP Transport Backend for Ethos
//!
//! This crate provides a concrete HTTP-based implementation of the
//! [`transport::Transport`] trait, enabling JSON-RPC communication
//! with Bitcoin Core and compatible nodes.
//!
//! ## Overview
//!
//! - Implements [`HttpTransport`], a thin wrapper over [`reqwest::Client`]
//! - Supports both authenticated and unauthenticated RPC calls
//! - Handles both single and batched JSON-RPC requests
//!
//! ## Example
//! ```no_run
//! use ethos_http::HttpTransport;
//! use transport::Transport;
//! use serde_json::json;
//! use std::path::PathBuf;
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let transport = HttpTransport::with_auth(
//!     "http://127.0.0.1:18443",
//!     "rpcuser",
//!     "rpcpassword",
//! );
//!
//! let result = transport.send("getblockchaininfo", &[]).await.unwrap();
//! println!("{:#?}", result);
//! # });
//! ```

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use async_trait::async_trait;
use serde_json::Value;
use transport::{BatchTransport, Transport, TransportError};

/// A concrete implementation of the [`Transport`] trait using HTTP.
///
/// This struct provides a thin wrapper around [`reqwest::Client`],
/// allowing JSON-RPC communication with a Bitcoin Core node (or any
/// compatible RPC endpoint). It supports both unauthenticated and
/// basic-authenticated connections.
///
/// Unlike higher-level clients, `HttpTransport` performs no schema
/// validation or result typing — it simply sends raw JSON-RPC requests
/// and returns the `result` field as a [`serde_json::Value`].
///
/// Errors encountered at any stage (HTTP, I/O, JSON parsing, or RPC)
/// are normalized into [`TransportError`] variants for uniform handling.
#[derive(Clone)]
pub struct HttpTransport {
    /// The underlying HTTP client used to perform requests.
    client: reqwest::Client,
    /// The full URL of the JSON-RPC endpoint (e.g. `http://127.0.0.1:18443`).
    url: String,
    /// Optional basic authentication credentials `(username, password)`.
    auth: Option<(String, String)>,
}

impl HttpTransport {
    /// Constructs a new `HttpTransport` targeting the provided URL.
    ///
    /// This variant does **not** use authentication.
    ///
    /// # Example
    /// ```
    /// use ethos_http::HttpTransport;
    /// use transport::Transport;
    ///
    /// let transport = HttpTransport::new("http://127.0.0.1:18443");
    /// assert_eq!(transport.endpoint(), "http://127.0.0.1:18443");
    /// ```
    pub fn new(url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), url: url.into(), auth: None }
    }

    /// Constructs a new `HttpTransport` with basic authentication.
    ///
    /// # Parameters
    /// - `url`: Target endpoint (e.g. `http://127.0.0.1:18443`)
    /// - `user`: RPC username
    /// - `pass`: RPC password
    ///
    /// # Example
    /// ```
    /// use ethos_http::HttpTransport;
    ///
    /// let transport = HttpTransport::with_auth(
    ///     "http://127.0.0.1:18443",
    ///     "rpcuser",
    ///     "rpcpassword",
    /// );
    /// ```
    pub fn with_auth(
        url: impl Into<String>,
        user: impl Into<String>,
        pass: impl Into<String>,
    ) -> Self {
        let url_string = url.into();
        logging::trace("HTTP", &format!("→ initializing HTTP transport for {}", url_string));
        Self {
            client: reqwest::Client::new(),
            url: url_string,
            auth: Some((user.into(), pass.into())),
        }
    }

    /// Constructs a new `HttpTransport` using credentials from a Bitcoin Core cookie file.
    ///
    /// The cookie file format is a single line containing `username:password`.
    /// The full path to the cookie file must be provided.
    ///
    /// # Parameters
    /// - `url`: Target endpoint (e.g. `http://127.0.0.1:18443`)
    /// - `cookie_path`: Path to the `.cookie` file
    ///
    /// # Errors
    /// Returns `TransportError::Other` if:
    /// - The cookie file cannot be read
    /// - The cookie file is empty or doesn't contain a colon
    ///
    /// # Example
    /// ```no_run
    /// use ethos_http::HttpTransport;
    /// use std::path::PathBuf;
    ///
    /// let transport = HttpTransport::from_cookie_file(
    ///     "http://127.0.0.1:18443",
    ///     PathBuf::from("/home/user/.bitcoin/.cookie"),
    /// )?;
    /// # Ok::<(), transport::TransportError>(())
    /// ```
    pub fn from_cookie_file(
        url: impl Into<String>,
        cookie_path: impl AsRef<Path>,
    ) -> Result<Self, TransportError> {
        let file = File::open(cookie_path.as_ref()).map_err(|e| {
            TransportError::Other(format!("Failed to read cookie file: {}", e))
        })?;

        let line = BufReader::new(file)
            .lines()
            .next()
            .ok_or_else(|| TransportError::Other("Cookie file is empty".to_string()))?
            .map_err(|e| TransportError::Other(format!("Failed to read cookie file: {}", e)))?;

        let colon = line.find(':').ok_or_else(|| {
            TransportError::Other("Invalid cookie file format: missing colon".to_string())
        })?;

        let user = line[..colon].to_string();
        let pass = line[colon + 1..].to_string();

        Ok(Self::with_auth(url, user, pass))
    }
}

#[async_trait]
impl Transport for HttpTransport {
    /// Sends a single JSON-RPC request and returns its `result` field as JSON.
    ///
    /// The request body is formatted according to the Bitcoin Core
    /// JSON-RPC 2.0 specification. If the server response includes a
    /// non-null `"error"` field, that is treated as a [`TransportError::Rpc`].
    ///
    /// If no `"result"` field is found, or the response body cannot be parsed,
    /// the error is wrapped in [`TransportError::InvalidFormat`] or
    /// [`TransportError::Serialization`], respectively.
    ///
    /// # Errors
    /// - [`TransportError::Http`] if the HTTP request fails
    /// - [`TransportError::Serialization`] if body parsing fails
    /// - [`TransportError::Rpc`] if the RPC returns a non-null error object
    /// - [`TransportError::InvalidFormat`] if the response is not a valid JSON-RPC envelope
    async fn send(&self, method: &str, params: &[Value]) -> Result<Value, TransportError> {
        logging::trace("HTTP", &format!("→ POST {} (method: {})", self.url, method));
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "ethos",
            "method": method,
            "params": params
        });

        let mut req = self.client.post(&self.url).json(&body);
        if let Some((u, p)) = &self.auth {
            req = req.basic_auth(u, Some(p));
        }
        let resp = req.send().await.map_err(|e| {
            tracing::error!("HTTP Transport - Request failed: {}", e);
            TransportError::Http(e.to_string())
        })?;

        let text = resp.text().await.map_err(|e| {
            tracing::error!("HTTP Transport - Failed to read body: {}", e);
            TransportError::Serialization(e.to_string())
        })?;

        let val: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| TransportError::Serialization(format!("{} (body: {})", e, text)))?;

        if let Some(error) = val.get("error") {
            if !error.is_null() {
                return Err(TransportError::Rpc(error.to_string()));
            }
        }

        if let Some(result) = val.get("result") {
            Ok(result.clone())
        } else {
            Err(TransportError::InvalidFormat(text))
        }
    }

    /// Returns the configured JSON-RPC endpoint URL.
    fn endpoint(&self) -> &str { &self.url }
}

/// Implements JSON-RPC batching per spec.
///
/// This allows multiple independent RPC calls to be packed into one POST.
/// Other transports (e.g. IPC, P2P) may not implement batching.
#[async_trait]
impl BatchTransport for HttpTransport {
    /// Sends a batch of JSON-RPC requests in a single HTTP call.
    ///
    /// Each element in `batch` should already be a valid JSON-RPC object.
    /// The server is expected to return an array of envelopes containing
    /// `"result"` and/or `"error"` fields. Results are extracted in order
    /// and returned as a `Vec<Value>`.
    ///
    /// # Errors
    /// - [`TransportError::Http`] if the request fails
    /// - [`TransportError::Serialization`] if the response cannot be parsed
    async fn send_batch(&self, batch: &[Value]) -> Result<Vec<Value>, TransportError> {
        let mut req = self.client.post(&self.url).json(batch);
        if let Some((u, p)) = &self.auth {
            req = req.basic_auth(u, Some(p));
        }
        let resp = req.send().await.map_err(|e| TransportError::Http(e.to_string()))?;

        let text = resp.text().await.map_err(|e| TransportError::Serialization(e.to_string()))?;

        let vals: Vec<Value> = serde_json::from_str(&text)
            .map_err(|e| TransportError::Serialization(format!("{} (batch body: {})", e, text)))?;

        let results: Vec<Value> =
            vals.into_iter().map(|v| v.get("result").cloned().unwrap_or(Value::Null)).collect();
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_new() {
        let url = "http://127.0.0.1:18443";
        let transport = HttpTransport::new(url);

        assert_eq!(transport.url, url);
        assert!(transport.auth.is_none());
        assert_eq!(transport.endpoint(), url);

        let transport_with_auth = HttpTransport::with_auth(url, "user", "pass");
        assert_eq!(transport_with_auth.endpoint(), url);
    }

    #[test]
    fn test_with_auth() {
        let url = "http://127.0.0.1:18443";
        let user = "rpcuser";
        let pass = "rpcpassword";
        let transport = HttpTransport::with_auth(url, user, pass);

        assert_eq!(transport.url, url);
        assert!(transport.auth.is_some());
        let (auth_user, auth_pass) = transport.auth.as_ref().expect("auth should be set");
        assert_eq!(auth_user, user);
        assert_eq!(auth_pass, pass);
        assert_eq!(transport.endpoint(), url);

        let url_string = String::from("http://127.0.0.1:18443");
        let user_string = String::from("rpcuser");
        let pass_string = String::from("rpcpassword");
        let transport2 = HttpTransport::with_auth(&url_string, &user_string, &pass_string);
        assert_eq!(transport2.url, url_string);
        assert!(transport2.auth.is_some());
    }

    #[tokio::test]
    async fn test_send() {
        let transport = HttpTransport::new("http://127.0.0.1:18443");
        let result = transport.send("getblockchaininfo", &[]).await;

        // Should fail with connection error, not return Ok(Default::default())
        assert!(result.is_err());

        // Test send_batch in the same test
        let batch = vec![json!({"jsonrpc": "2.0", "id": "1", "method": "test", "params": []})];
        let batch_result = transport.send_batch(&batch).await;

        // Should fail with connection error, not return Ok(vec![]) or Ok(vec![Default::default()])
        assert!(batch_result.is_err());
    }
}
