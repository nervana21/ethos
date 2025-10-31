use anyhow::Result;

mod bitcoin_core;
mod core_lightning;

#[tokio::main]
async fn main() -> Result<()> {
	println!("\n₿ Running Bitcoin Core RPC Test...");
	bitcoin_core::run_test().await?;

	// println!("\n⚡ Running Core Lightning RPC Test...");
	core_lightning::run_test().await?;

	println!("\n✅ All tests completed successfully!");
	Ok(())
}
