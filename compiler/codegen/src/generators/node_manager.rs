//! Node manager generator for Bitcoin protocol implementations.
//!
//! This module generates node manager code that can spawn and manage
//! Bitcoin protocol nodes (e.g. bitcoind) for testing.

use std::fmt::Write as _;

use ir::RpcDef;
use types::Implementation;

use crate::CodeGenerator;

/// Generator for creating node manager modules
pub struct NodeManagerGenerator {
    implementation: Implementation,
}

impl NodeManagerGenerator {
    /// Create a new node manager generator for the specified implementation
    pub fn new(implementation: Implementation) -> Self { Self { implementation } }
}

impl CodeGenerator for NodeManagerGenerator {
    fn generate(&self, _methods: &[RpcDef]) -> Vec<(String, String)> {
        let metadata = self.implementation.node_metadata();
        let node_manager_name = self.implementation.node_manager_name();
        let display_name = self.implementation.display_name();

        let mut code = String::new();

        // Generate module header and imports
        generate_module_header(&mut code, display_name);
        generate_imports(&mut code);

        // Generate common structures
        generate_node_state_struct(&mut code);
        generate_port_selection_enum(&mut code);

        // Generate trait
        generate_node_manager_trait(&mut code);

        // Generate implementation
        generate_node_manager_struct(&mut code, node_manager_name);
        generate_node_manager_impl(&mut code, node_manager_name);
        generate_trait_impl(&mut code, node_manager_name, &metadata);

        vec![("node_manager.rs".to_string(), code)]
    }
}

fn generate_module_header(code: &mut String, display_name: &str) {
    writeln!(
        code,
        "//! Node module for {} RPC testing
//!
//! This module provides utilities for managing {} nodes in test environments.",
        display_name, display_name
    )
    .expect("Failed to write module header");
}

fn generate_imports(code: &mut String) {
    writeln!(
        code,
        r#"
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tempfile::TempDir;
use tokio::process::{{Child, Command}};
use tokio::sync::{{Mutex, RwLock}};
use std::process::Stdio;

use crate::test_config::TestConfig;
use crate::transport::{{TransportError, core::TransportExt}};"#
    )
    .expect("Failed to write imports");

    writeln!(
        code,
        r#"
use tracing::{{info, debug, error}};
use tokio::io::AsyncBufReadExt;
use std::time::Instant;
use crate::transport::DefaultTransport;"#
    )
    .expect("Failed to write tracing imports");
}

fn generate_node_state_struct(code: &mut String) {
    writeln!(
        code,
        r#"
/// Represents the current state of a node
#[derive(Debug, Default, Clone)]
pub struct NodeState {{
    /// Whether the node is currently running
    pub is_running: bool,
}}"#
    )
    .expect("Failed to write NodeState struct");
}

fn generate_port_selection_enum(code: &mut String) {
    writeln!(
        code,
        r#"
/// Configuration for port selection behavior
#[derive(Debug, Clone)]
pub enum PortSelection {{
    /// Use the specified port number
    Fixed(u16),
    /// Let the OS assign an available port
    Dynamic,
    /// Use port 0 (not recommended, may cause daemon to fail)
    Zero,
}}"#
    )
    .expect("Failed to write PortSelection enum");
}

fn generate_node_manager_trait(code: &mut String) {
    writeln!(
        code,
        r#"
/// Trait defining the interface for a node manager
#[async_trait]
pub trait NodeManager: Send + Sync + std::any::Any + std::fmt::Debug {{
    async fn start(&self) -> Result<(), TransportError>;
    async fn stop(&mut self) -> Result<(), TransportError>;
    async fn get_state(&self) -> Result<NodeState, TransportError>;"#
    )
    .expect("Failed to write trait start");

    writeln!(
        code,
        r#"    /// Return the RPC port this manager was configured with
    fn rpc_port(&self) -> u16;
    /// Return the RPC username this manager was configured with
    fn rpc_username(&self) -> &str;
    /// Return the RPC password this manager was configured with
    fn rpc_password(&self) -> &str;"#
    )
    .expect("Failed to write trait methods");

    writeln!(
        code,
        r#"    /// Create a transport for communicating with the node
    async fn create_transport(&self) -> Result<std::sync::Arc<crate::transport::DefaultTransport>, TransportError>;
}}"#
    ).expect("Failed to write trait end");
}

fn generate_node_manager_struct(code: &mut String, node_manager_name: &str) {
    writeln!(
        code,
        r#"
/// Implementation of the node manager
#[derive(Debug)]
pub struct {} {{
    /// Shared state of the node
    state: Arc<RwLock<NodeState>>,
    /// Child process handle for the daemon
    child: Arc<Mutex<Option<Child>>>,
    /// RPC port for communication with the node
    pub rpc_port: u16,"#,
        node_manager_name
    )
    .expect("Failed to write struct start");

    writeln!(
        code,
        r#"    /// Test configuration for the node
    config: TestConfig,
    /// Temporary directory for node data (cleaned up on drop)
    _datadir: Option<TempDir>,"#
    )
    .expect("Failed to write struct fields");

    writeln!(code, r#"}}"#).expect("Failed to write struct end");
}

fn generate_node_manager_impl(code: &mut String, node_manager_name: &str) {
    writeln!(
        code,
        r#"
impl {} {{"#,
        node_manager_name
    )
    .expect("Failed to write impl start");

    // Generate constructor methods
    writeln!(
        code,
        r#"    /// Create a new node manager with default configuration
    pub fn new() -> Result<Self, TransportError> {{
        Self::new_with_config(&TestConfig::default())
    }}

    /// Create a new node manager with custom configuration
    pub fn new_with_config(config: &TestConfig) -> Result<Self, TransportError> {{
        let datadir = TempDir::new()?;

        // Handle automatic port selection
        let rpc_port = if config.rpc_port == 0 {{
            // Get a random free port by binding to 127.0.0.1:0
            // The listener is dropped at the end of the block, freeing the port
            {{
                let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
                listener.local_addr()?.port()
            }}
        }} else {{
            config.rpc_port
        }};"#
    )
    .expect("Failed to write constructor start");

    writeln!(
        code,
        r#"
        Ok(Self {{
            state: Arc::new(RwLock::new(NodeState::default())),
            child: Arc::new(Mutex::new(None)),
            rpc_port,
            config: config.clone(),
            _datadir: Some(datadir),
        }})"#
    )
    .expect("Failed to write constructor");

    writeln!(
        code,
        r#"
    }}

    /// Get the RPC port for this node manager
    pub fn rpc_port(&self) -> u16 {{ self.rpc_port }}

    /// Gets the test configuration used by this node manager
    pub fn config(&self) -> &TestConfig {{ &self.config }}"#
    )
    .expect("Failed to write rpc_port method");

    writeln!(
        code,
        r#"
    /// Get the RPC username from the configuration
    pub fn rpc_username(&self) -> &str {{ &self.config.rpc_username }}

    /// Get the RPC password from the configuration
    pub fn rpc_password(&self) -> &str {{ &self.config.rpc_password }}"#
    )
    .expect("Failed to write auth methods");

    writeln!(
        code,
        r#"
}}"#
    )
    .expect("Failed to write impl end");
}

fn generate_trait_impl(
    code: &mut String,
    node_manager_name: &str,
    metadata: &types::node_metadata::NodeMetadata,
) {
    writeln!(
        code,
        r#"
#[async_trait]
impl NodeManager for {} {{"#,
        node_manager_name
    )
    .expect("Failed to write trait impl start");

    // Generate start method
    generate_start_method(code, metadata);

    // Generate stop method
    generate_stop_method(code, metadata);

    // Generate get_state method
    writeln!(
        code,
        r#"
    async fn get_state(&self) -> Result<NodeState, TransportError> {{
        Ok(self.state.read().await.clone())
    }}"#
    )
    .expect("Failed to write get_state method");

    writeln!(
        code,
        r#"
    fn rpc_port(&self) -> u16 {{ self.rpc_port }}

    fn rpc_username(&self) -> &str {{ &self.config.rpc_username }}

    fn rpc_password(&self) -> &str {{ &self.config.rpc_password }}"#
    )
    .expect("Failed to write trait methods");

    // Generate create_transport method
    generate_create_transport_method(code, metadata);

    writeln!(
        code,
        r#"
}}"#
    )
    .expect("Failed to write trait impl end");
}

fn generate_start_method(code: &mut String, metadata: &types::node_metadata::NodeMetadata) {
    writeln!(
        code,
        r#"
    async fn start(&self) -> Result<(), TransportError> {{
        let mut state = self.state.write().await;
        if state.is_running {{
            return Ok(());
        }}"#
    )
    .expect("Failed to write start method start");

    generate_http_start_logic(code, metadata);

    writeln!(
        code,
        r#"
    }}"#
    )
    .expect("Failed to write start method end");
}

fn generate_http_start_logic(code: &mut String, metadata: &types::node_metadata::NodeMetadata) {
    writeln!(
        code,
        r#"
        let datadir = self._datadir.as_ref().unwrap().path();
        let mut cmd = Command::new("{}");

        let chain = format!("-chain={{}}", self.config.as_chain_str());
        let data_dir = format!("-datadir={{}}", datadir.display());
        let rpc_port = format!("-rpcport={{}}", self.rpc_port);
        let rpc_bind = format!("-rpcbind=127.0.0.1:{{}}", self.rpc_port);
        let rpc_user = format!("-rpcuser={{}}", self.config.rpc_username);
        let rpc_password = format!("-rpcpassword={{}}", self.config.rpc_password);

        let mut args = vec![
            &chain,
            "-listen=0",
            &data_dir,
            &rpc_port,
            &rpc_bind,
            "-rpcallowip=127.0.0.1",
            "-fallbackfee=0.0002",
            "-server=1",
            "-prune=1",
            &rpc_user,
            &rpc_password,
        ];

        for arg in &self.config.extra_args {{
            args.push(arg);
        }}

        cmd.args(&args);

        // Capture both stdout and stderr for better error reporting
        cmd.stderr(Stdio::piped());
        cmd.stdout(Stdio::piped());

        let mut child = cmd.spawn()?;

        // Read stderr in a separate task
        let stderr = child.stderr.take().unwrap();
        let stderr_reader = tokio::io::BufReader::new(stderr);
        tokio::spawn(async move {{
            let mut lines = stderr_reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {{
                error!("{} stderr: {{}}", line);
            }}
        }});

        // Read stdout in a separate task
        let stdout = child.stdout.take().unwrap();
        let stdout_reader = tokio::io::BufReader::new(stdout);
        tokio::spawn(async move {{
            let mut lines = stdout_reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {{
                info!("{} stdout: {{}}", line);
            }}
        }});

        // Store the child process
        let mut child_guard = self.child.lock().await;
        *child_guard = Some(child);

        info!("Waiting for {} node to initialize...");
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Create transport for RPC health check
        let transport = DefaultTransport::new(
            format!("http://127.0.0.1:{{}}/", self.rpc_port),
            Some((self.config.rpc_username.clone(), self.config.rpc_password.clone())),
        );

        // Wait for node to be ready
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut attempts = 0;
        while Instant::now() < deadline {{
            if let Some(child) = child_guard.as_mut() {{
                if let Ok(Some(status)) = child.try_wait() {{
                    let error = format!("{} node exited early with status: {{}}", status);
                    error!("{{}}", error);
                    return Err(TransportError::Rpc(error));
                }}
            }}

            // Try to connect to RPC
            match transport.call::<serde_json::Value>("{}", &[]).await {{
                Ok(_) => {{
                    state.is_running = true;
                    info!("{} node started successfully on port {{}}", self.rpc_port);
                    return Ok(());
                }}
                Err(e) => {{
                    debug!("Failed to connect to RPC (attempt {{}}): {{}}", attempts, e);
                }}
            }}

            attempts += 1;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }}

        let error = format!(
            "Timed out waiting for {} node to start on port {{}} after {{}} attempts",
            self.rpc_port, attempts
        );
        error!("{{}}", error);
        return Err(TransportError::Rpc(error));"#,
        metadata.executable,
        metadata.executable,
        metadata.executable,
        metadata.executable,
        metadata.executable,
        metadata.readiness_method,
        metadata.executable,
        metadata.executable
    )
    .expect("Failed to write HTTP start logic");
}

fn generate_stop_method(code: &mut String, metadata: &types::node_metadata::NodeMetadata) {
    writeln!(
        code,
        r#"
    async fn stop(&mut self) -> Result<(), TransportError> {{
        let mut state = self.state.write().await;
        if !state.is_running {{
            return Ok(());
        }}

        let mut child = self.child.lock().await;
        if let Some(mut child_process) = child.take() {{
            info!("Stopping {} node...");
            let _ = child_process.kill().await;
        }}

        state.is_running = false;
        info!("{} node stopped");
        Ok(())
    }}"#,
        metadata.executable, metadata.executable
    )
    .expect("Failed to write stop method");
}

fn generate_create_transport_method(
    code: &mut String,
    metadata: &types::node_metadata::NodeMetadata,
) {
    writeln!(
        code,
        r#"
    async fn create_transport(&self) -> Result<std::sync::Arc<crate::transport::DefaultTransport>, TransportError> {{
        use std::sync::Arc;
        use crate::transport::DefaultTransport;

        // Create HTTP transport for Bitcoin Core
        let rpc_url = format!("http://127.0.0.1:{{}}", self.rpc_port());
        let auth = Some((self.rpc_username().to_string(), self.rpc_password().to_string()));
        let transport = Arc::new(DefaultTransport::new(rpc_url, auth));

        // Wait for node to be ready for RPC with Bitcoin Core specific initialization logic
        // Bitcoin Core initialization states that require waiting:
        // -28: RPC in warmup
        // -4:  RPC in warmup (alternative code)
        let init_states = [
            "\"code\":-28",
            "\"code\":-4",
        ];

        let max_retries = 30;
        let mut retries = 0;

        loop {{
            match transport.call::<serde_json::Value>("{}", &[]).await {{
                Ok(_) => break,
                Err(TransportError::Rpc(e)) => {{
                    // Check if the error matches any known initialization state
                    let is_init_state = init_states.iter().any(|state| e.contains(state));
                    if is_init_state && retries < max_retries {{
                        tracing::debug!("Waiting for initialization: {{}} (attempt {{}}/{{}})", e, retries + 1, max_retries);
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        retries += 1;
                        continue;
                    }}
                    return Err(TransportError::Rpc(e));
                }}
                Err(e) => return Err(e),
            }}
        }}

        if retries > 0 {{
            tracing::debug!("Node initialization completed after {{}} attempts", retries);
        }}

        Ok(transport)
    }}"#,
        metadata.readiness_method
    )
    .expect("Failed to write create_transport method");
}
