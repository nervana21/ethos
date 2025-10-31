//! End-to-end tests for Ethos adapters.
//!
//! This crate contains integration tests that verify the functionality
//! of various blockchain node adapters, including Bitcoin Core and
//! Core Lightning RPC interfaces.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

use anyhow::Result;

mod bitcoin_core;
mod core_lightning;

#[tokio::main]
async fn main() -> Result<()> {
	println!("\n₿ Running Bitcoin Core RPC Test...");
	bitcoin_core::run_test().await?;

	println!("\n⚡ Running Core Lightning RPC Test...");
	core_lightning::run_test().await?;

	println!("\n✅ All tests completed successfully!");
	Ok(())
}
