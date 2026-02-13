//! End-to-end tests for Ethos.
//!
//! This crate contains integration tests that verify the generated
//! Bitcoin Core RPC client works correctly with a running node.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

use anyhow::Result;

mod bitcoin_core;

#[tokio::main]
async fn main() -> Result<()> {
	println!("\n₿ Running Bitcoin Core RPC Test...");
	bitcoin_core::run_test().await?;

	println!("\n✅ All tests completed successfully!");
	Ok(())
}
