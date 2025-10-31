use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::Result;
use core_lightning_client_rpc_25_09_1::{CoreLightningNodeManager, TestConfig};
use core_lightning_client_rpc_25_09_1::node::NodeManager;
use core_lightning_client_rpc_25_09_1::transport::TransportTrait;

struct TestCleanup {
	bitcoin_child: Option<std::process::Child>,
	bitcoin_dir: PathBuf,
	bitcoin_rpc_port: u16,
}

impl Drop for TestCleanup {
	fn drop(&mut self) {
		// Stop Bitcoin
		if let Some(ref mut child) = self.bitcoin_child {
			println!("🔍 Stopping Bitcoin Core...");
			// Try graceful shutdown via RPC
			let _ = Command::new("bitcoin-cli")
				.arg("-regtest")
				.arg(format!("-datadir={}", self.bitcoin_dir.display()))
				.arg(format!("-rpcport={}", self.bitcoin_rpc_port))
				.arg("-rpcuser=rpcuser")
				.arg("-rpcpassword=rpcpassword")
				.arg("stop")
				.output();

			// Wait for graceful shutdown
			std::thread::sleep(Duration::from_millis(1000));

			// Force kill if still running
			let _ = child.kill();
		}

		// Wait for ports to be released
		std::thread::sleep(Duration::from_millis(2000));

		// Clean up Bitcoin directory
		let _ = std::fs::remove_dir_all(&self.bitcoin_dir);

		println!("✅ Cleanup completed");
	}
}

// NOTE: Removed global orphaned-process cleanup to avoid interfering with production.

pub async fn run_test() -> Result<()> {
	println!("\n=== Step-by-Step Lightning RPC Test ===");

	// Do NOT clean up globally; tests manage only their own child processes

	let bitcoin_dir = temp_bitcoin_datadir()?;
	std::fs::create_dir_all(&bitcoin_dir)?;

	// Find available ports for Bitcoin
	let (bitcoin_rpc_port, bitcoin_p2p_port) = find_available_ports(18447, 18448).await?;

	// Create cleanup guard
	let mut cleanup =
		TestCleanup { bitcoin_child: None, bitcoin_dir: bitcoin_dir.clone(), bitcoin_rpc_port };

	// Step 1: Start Bitcoin Core for Lightning test
	println!("\n🔍 Step 1: Starting Bitcoin Core for Lightning test...");

	let bitcoin_child = Command::new("bitcoind")
		.arg("-regtest")
		.arg(format!("-datadir={}", bitcoin_dir.display()))
		.arg("-server=1")
		.arg("-rpcuser=rpcuser")
		.arg("-rpcpassword=rpcpassword")
		.arg("-rpcbind=127.0.0.1")
		.arg("-rpcallowip=127.0.0.1")
		.arg(format!("-rpcport={}", bitcoin_rpc_port))
		.arg(format!("-port={}", bitcoin_p2p_port))
		.arg("-listen=0")
		.arg("-dnsseed=0")
		.arg("-discover=0")
		.arg("-fallbackfee=0.0001")
		.arg("-printtoconsole=1")
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.map_err(|e| {
			anyhow::anyhow!(
				"Failed to start bitcoind for Lightning test. Is it installed and on PATH? {e}"
			)
		})?;

	cleanup.bitcoin_child = Some(bitcoin_child);

	// Wait for Bitcoin Core to start
	tokio::time::sleep(Duration::from_millis(1000)).await;

	// Check if the process is still running
	match cleanup.bitcoin_child.as_mut().unwrap().try_wait() {
		Ok(Some(status)) => {
			// Capture stderr to see what went wrong
			let stderr = cleanup.bitcoin_child.as_mut().unwrap().stderr.take();
			if let Some(mut stderr) = stderr {
				let mut stderr_output = String::new();
				use std::io::Read;
				let _ = stderr.read_to_string(&mut stderr_output);
				eprintln!("bitcoind stderr: {}", stderr_output);
			}
			return Err(anyhow::anyhow!("bitcoind exited early with status: {}", status));
		},
		Ok(None) => {
			println!("✅ Bitcoin Core started successfully for Lightning test");
		},
		Err(e) => {
			return Err(anyhow::anyhow!("Failed to check bitcoind status: {}", e));
		},
	}

	// Wait for Bitcoin Core RPC to be ready (scoped to our datadir/port)
	println!("🔍 Waiting for Bitcoin Core RPC to be ready...");
	let bitcoin_rpc_url = format!("http://127.0.0.1:{}", bitcoin_rpc_port);
	wait_for_bitcoin_rpc_ready(&bitcoin_rpc_url, 30, Duration::from_millis(500)).await?;
	println!("✅ Bitcoin Core RPC is ready");

	// Generate initial blocks for Lightning to work with
	println!("🔍 Generating initial blocks for Lightning...");
	let _ = Command::new("bitcoin-cli")
		.arg("-regtest")
		.arg(format!("-datadir={}", bitcoin_dir.display()))
		.arg(format!("-rpcport={}", bitcoin_rpc_port))
		.arg("-rpcuser=rpcuser")
		.arg("-rpcpassword=rpcpassword")
		.arg("generatetoaddress")
		.arg("101") // Generate 101 blocks to ensure we're past the initial download
		.arg("bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4") // A valid bech32 address for regtest
		.output();
	println!("✅ Initial blocks generated");

	// Step 2: Create Lightning test client with Bitcoin connection
	println!("\n🔍 Step 2: Creating Lightning test client...");

	// Configure Lightning to connect to our Bitcoin node using explicit credentials
	let mut config = TestConfig::default();
	config.extra_args.extend([
		"--bitcoin-rpcuser=rpcuser".to_string(),
		"--bitcoin-rpcpassword=rpcpassword".to_string(),
		"--bitcoin-rpcconnect=127.0.0.1".to_string(),
		format!("--bitcoin-rpcport={}", bitcoin_rpc_port),
	]);

	let node_manager = CoreLightningNodeManager::new_with_config(&config)?;

	// Start the Core Lightning node first
	println!("Starting Core Lightning node...");
	node_manager.start().await?;

	let transport: std::sync::Arc<dyn TransportTrait> = node_manager.create_transport().await?;
	let client = transport;
	println!("✅ Lightning test client created successfully");

	tokio::time::sleep(Duration::from_millis(2000)).await;

	let info = client.send_request("getinfo", &[]).await?;
	println!("Node info: {:#?}", info);

	let peers = client.send_request("listpeers", &[
		serde_json::Value::Null,
		serde_json::Value::Null
	]).await?;
	println!("Peers: {:#?}", peers);

	let funds = client.send_request("listfunds", &[
		serde_json::Value::Bool(false)
	]).await?;
	println!("Funds: {:#?}", funds);

	let address = client.send_request("newaddr", &[
		serde_json::Value::String("bech32".to_string())
	]).await?;
	println!("Address: {:#?}", address);

	println!("\n=== Lightning Test Completed Successfully ===\n");
	Ok(())
}

async fn wait_for_bitcoin_rpc_ready(
	rpc_url: &str, max_tries: usize, delay: Duration,
) -> Result<()> {
	let port = rpc_url.split(':').next_back().unwrap_or("18443");

	for _i in 0..max_tries {
		let output = Command::new("bitcoin-cli")
			.arg("-regtest")
			.arg(format!("-rpcport={}", port))
			.arg("-rpcuser=rpcuser")
			.arg("-rpcpassword=rpcpassword")
			.arg("getblockchaininfo")
			.output();

		if let Ok(output) = output {
			if output.status.success() {
				return Ok(());
			}
		}
		tokio::time::sleep(delay).await;
	}
	Err(anyhow::anyhow!("Bitcoin RPC not ready after {} attempts", max_tries))
}

fn temp_bitcoin_datadir() -> Result<std::path::PathBuf> {
	let mut dir = std::env::temp_dir();
	let unique = format!("lightning-btc-test-{}-{}", std::process::id(), chrono_like_now());
	dir.push(unique);
	Ok(dir)
}

fn chrono_like_now() -> String {
	let now = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap_or_else(|_| std::time::Duration::from_secs(0));
	format!("{}", now.as_secs())
}

async fn find_available_ports(start_rpc_port: u16, start_p2p_port: u16) -> Result<(u16, u16)> {
	use std::net::{SocketAddr, TcpListener};

	// Try to find available ports starting from the requested ports
	for offset in 0..100 {
		let rpc_port = start_rpc_port + offset;
		let p2p_port = start_p2p_port + offset;

		// Check if RPC port is available
		if TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], rpc_port))).is_ok() {
			// Check if P2P port is available
			if TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], p2p_port))).is_ok() {
				return Ok((rpc_port, p2p_port));
			}
		}
	}

	Err(anyhow::anyhow!("Could not find available ports after checking 100 combinations"))
}
