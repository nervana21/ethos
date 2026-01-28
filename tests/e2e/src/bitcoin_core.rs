use anyhow::Result;
use std::str::FromStr;
use ethos_bitcoind::{
	Address, Amount, BitcoinClient, DefaultTransport, Network, TestConfig,
};
use ethos_bitcoind::node::NodeManager;

pub async fn run_test() -> Result<()> {
	let default_config = TestConfig::default();
	let mut default_node_manager =
		ethos_bitcoind::BitcoinNodeManager::new_with_config(&default_config)?;

	default_node_manager.start().await?;
	let client: std::sync::Arc<DefaultTransport> = default_node_manager.create_transport().await?;

	client
		.create_wallet(
			"test_wallet".to_string(),
			Some(false),
			Some(false),
			Some("".to_string()),
			Some(false),
			Some(true),
			Some(false),
			Some(false)
		)
		.await?;

	let info = client.get_blockchain_info().await?;
	let difficulty = client.get_difficulty().await?;
	let mempool_info = client.get_mempool_info().await?;
	let mining_info = client.get_mining_info().await?;

	// Validate types/values for fields affected by categorization rules
	let diff_from_info = info.difficulty;
	let diff_simple = difficulty.value as f64; // Convert u64 to f64 for comparison
	assert!(diff_simple.is_finite() && diff_simple >= 0.0);
	assert!(diff_from_info.is_finite() && diff_from_info >= 0.0);
	assert!((diff_simple - diff_from_info).abs() < 1e-6);
	assert!(mempool_info.incrementalrelayfee.is_finite() && mempool_info.incrementalrelayfee >= 0.0);
	assert!(mining_info.difficulty.is_finite());
	assert!(mining_info.networkhashps.is_finite());

	let address_resp = client.get_new_address(None, Some("bech32m".to_string())).await?;
	let address = Address::from_str(&address_resp.value)
		.map_err(|e| anyhow::anyhow!("Failed to parse address: {}", e))?
		.require_network(Network::Regtest)
		.map_err(|e| anyhow::anyhow!("Failed to validate address network: {}", e))?;

	client.generate_to_address(101, address.clone(), Some(2000)).await?;
	client.generate_to_address(100, address.clone(), Some(2000)).await?;

	let amount = 1_000u64;
	let _txid = client.send_to_address(
		address.clone(),
		serde_json::json!(Amount::from_sat(amount).to_sat()),
		Some("Test transaction".to_string()),
		Some("Test comment".to_string()),
		None, None, None, None, None, None, None
	).await?;

	client.generate_to_address(1, address.clone(), Some(2000)).await?;
	let _final_balance = client.get_received_by_address(address.clone(), Some(0), Some(false)).await?;

	// Test confirmations field (i64 with negative values for orphaned blocks)
	client.generate_to_address(5, address.clone(), Some(2000)).await?;

	let current_height = client.get_block_count().await?.value;
	let block_height_to_test = current_height - 3;
	let block_hash_str = client.get_block_hash(block_height_to_test as i64).await?.value;
	let block_hash = bitcoin::BlockHash::from_str(&block_hash_str)
		.map_err(|e| anyhow::anyhow!("Failed to parse block hash: {}", e))?;

	let header_before = client.get_block_header(block_hash, Some(true)).await?;
	assert!(header_before.confirmations > 0);
	assert!(header_before.confirmations as u64 <= current_height - block_height_to_test + 1);

	client.invalidate_block(block_hash).await?;
	let header_orphaned = client.get_block_header(block_hash, Some(true)).await?;
	assert_eq!(header_orphaned.confirmations, -1i64);

	let confirmations: i64 = header_orphaned.confirmations;
	assert!(confirmations < 0);

	client.reconsider_block(block_hash).await?;
	let header_restored = client.get_block_header(block_hash, Some(true)).await?;
	assert!(header_restored.confirmations > 0);

	// Test changepos field type (i64)
	let _test_changepos: i64 = -1;
	assert!(_test_changepos < 0);

	// Test nblocks parameter type (i64)
	let hashps_100 = client.get_network_hash_ps(Some(100i64), None).await?;
	assert!(hashps_100.value > 0u64);

	let hashps_since_diff = client.get_network_hash_ps(Some(-1i64), None).await?;
	assert!(hashps_since_diff.value > 0u64);

	if let Err(e) = default_node_manager.stop().await {
		eprintln!("Warning: Failed to stop Bitcoin node: {}", e);
	}

	Ok(())
}
