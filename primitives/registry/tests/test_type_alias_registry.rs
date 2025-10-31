use std::collections::HashMap;
use std::path::Path;

use registry::TypeAliasRegistry;

#[test]
fn test_new() {
    let mut map = HashMap::new();
    map.insert("TxInput".to_string(), "TransactionInput".to_string());
    map.insert("TxOutput".to_string(), "TransactionOutput".to_string());

    let registry = TypeAliasRegistry::new(map);

    assert_eq!(registry.len(), 2);
    assert!(!registry.is_empty());
    assert_eq!(registry.resolve("TxInput"), "TransactionInput");
    assert_eq!(registry.resolve("TxOutput"), "TransactionOutput");
}

#[test]
fn test_load_from_file() {
    let test_file = Path::new("test_promotion_map.json");

    if test_file.exists() {
        let registry =
            TypeAliasRegistry::load_from_file(test_file).expect("Failed to load type alias map");

        assert_eq!(registry.len(), 3);
        assert!(!registry.is_empty());

        let resolved = registry.resolve("TxInput");
        assert!(!resolved.is_empty());
    } else {
        // Test error case when file doesn't exist
        let result = TypeAliasRegistry::load_from_file("nonexistent_file.json");
        assert!(result.is_err());
    }
}

#[test]
fn test_resolve() {
    let mut map = HashMap::new();
    map.insert("BlockHash".to_string(), "Hash".to_string());
    map.insert("TxInput".to_string(), "TransactionInput".to_string());
    map.insert("TxOutput".to_string(), "TransactionOutput".to_string());

    let registry = TypeAliasRegistry::new(map);

    assert_eq!(registry.resolve("BlockHash"), "Hash");
    assert_eq!(registry.resolve("TxInput"), "TransactionInput");
    assert_eq!(registry.resolve("TxOutput"), "TransactionOutput");

    assert_eq!(registry.resolve("Hash"), "Hash");
    assert_eq!(registry.resolve("TransactionInput"), "TransactionInput");
    assert_eq!(registry.resolve("TransactionOutput"), "TransactionOutput");
    assert_eq!(registry.resolve("UnknownType"), "UnknownType");
    assert_eq!(registry.resolve("SomeOtherType"), "SomeOtherType");

    assert!(!registry.resolve("BlockHash").is_empty());
    assert!(!registry.resolve("TxInput").is_empty());
    assert!(!registry.resolve("TxOutput").is_empty());
    assert!(!registry.resolve("UnknownType").is_empty());
}

#[test]
fn test_is_alias() {
    let mut map = HashMap::new();
    map.insert("BlockHash".to_string(), "Hash".to_string());
    map.insert("TxInput".to_string(), "TransactionInput".to_string());
    map.insert("TxOutput".to_string(), "TransactionOutput".to_string());

    let registry = TypeAliasRegistry::new(map);

    assert!(registry.is_alias("BlockHash"));
    assert!(registry.is_alias("TxInput"));
    assert!(registry.is_alias("TxOutput"));

    assert!(!registry.is_alias("UnknownType"));
    assert!(!registry.is_alias("SomeOtherType"));

    assert!(!registry.is_alias("Hash"));
    assert!(!registry.is_alias("TransactionInput"));
    assert!(!registry.is_alias("TransactionOutput"));
}

#[test]
fn test_len() {
    let empty_registry = TypeAliasRegistry::new(HashMap::new());
    assert_eq!(empty_registry.len(), 0);

    let mut map1 = HashMap::new();
    map1.insert("TxInput".to_string(), "TransactionInput".to_string());
    let registry1 = TypeAliasRegistry::new(map1);
    assert_eq!(registry1.len(), 1);

    let mut map3 = HashMap::new();
    map3.insert("BlockHash".to_string(), "Hash".to_string());
    map3.insert("TxInput".to_string(), "TransactionInput".to_string());
    map3.insert("TxOutput".to_string(), "TransactionOutput".to_string());
    let registry3 = TypeAliasRegistry::new(map3);
    assert_eq!(registry3.len(), 3);
}

#[test]
fn test_is_empty() {
    let empty_registry = TypeAliasRegistry::new(HashMap::new());
    assert!(empty_registry.is_empty());

    let mut map = HashMap::new();
    map.insert("TxInput".to_string(), "TransactionInput".to_string());
    let registry = TypeAliasRegistry::new(map);
    assert!(!registry.is_empty());

    let mut map_multi = HashMap::new();
    map_multi.insert("TxInput".to_string(), "TransactionInput".to_string());
    map_multi.insert("TxOutput".to_string(), "TransactionOutput".to_string());
    let registry_multi = TypeAliasRegistry::new(map_multi);
    assert!(!registry_multi.is_empty());
}

#[test]
fn test_get_canonical() {
    let mut map = HashMap::new();
    map.insert("BlockHash".to_string(), "Hash".to_string());
    map.insert("TxInput".to_string(), "TransactionInput".to_string());
    map.insert("TxOutput".to_string(), "TransactionOutput".to_string());

    let registry = TypeAliasRegistry::new(map);

    assert_eq!(registry.get_canonical("BlockHash"), Some("Hash"));
    assert_eq!(registry.get_canonical("TxInput"), Some("TransactionInput"));
    assert_eq!(registry.get_canonical("TxOutput"), Some("TransactionOutput"));

    assert_eq!(registry.get_canonical("UnknownType"), None);
    assert_eq!(registry.get_canonical("SomeOtherType"), None);

    assert_eq!(registry.get_canonical("TransactionInput"), None);
    assert_eq!(registry.get_canonical("Hash"), None);
}

#[test]
fn test_validate_types() {
    let mut map = HashMap::new();
    // Bitcoin types
    map.insert("BlockHash".to_string(), "Hash".to_string());
    map.insert("TxId".to_string(), "Hash".to_string());
    map.insert("TxInput".to_string(), "TransactionInput".to_string());
    map.insert("TxOutput".to_string(), "TransactionOutput".to_string());

    // Simple Bitcoin Core types
    map.insert("BitcoinBlockHash".to_string(), "bitcoin::BlockHash".to_string());
    map.insert("BitcoinTxid".to_string(), "bitcoin::Txid".to_string());
    map.insert("BitcoinAmount".to_string(), "bitcoin::Amount".to_string());
    map.insert("BitcoinAddress".to_string(), "bitcoin::Address".to_string());
    // Complex Bitcoin Core types
    map.insert("HashOrHeight".to_string(), "HashOrHeight".to_string());
    map.insert("TxidArray".to_string(), "Vec<bitcoin::Txid>".to_string());
    map.insert("StringArray".to_string(), "Vec<String>".to_string());
    map.insert(
        "BitcoinObject".to_string(),
        "serde_json::Map<String, serde_json::Value>".to_string(),
    );
    map.insert(
        "BitcoinObjectArray".to_string(),
        "Vec<serde_json::Map<String, serde_json::Value>>".to_string(),
    );

    // Lightning Network types
    map.insert("PublicKey".to_string(), "PublicKey".to_string());
    map.insert("ShortChannelId".to_string(), "ShortChannelId".to_string());
    map.insert("Satoshis".to_string(), "u64".to_string());
    map.insert("MilliSatoshis".to_string(), "u64".to_string());

    // Core Lightning specific types
    map.insert("Amount".to_string(), "u64".to_string());
    map.insert("Msat".to_string(), "u64".to_string());

    // LND specific types
    map.insert("Bytes".to_string(), "Vec<u8>".to_string());

    let registry = TypeAliasRegistry::new(map);

    let known_aliases = vec![
        "BlockHash".to_string(),
        "TxId".to_string(),
        "TxInput".to_string(),
        "TxOutput".to_string(),
        "BitcoinBlockHash".to_string(),
        "BitcoinTxid".to_string(),
        "BitcoinAmount".to_string(),
        "BitcoinAddress".to_string(),
        "HashOrHeight".to_string(),
        "TxidArray".to_string(),
        "StringArray".to_string(),
        "BitcoinObject".to_string(),
        "BitcoinObjectArray".to_string(),
        "PublicKey".to_string(),
        "ShortChannelId".to_string(),
        "Satoshis".to_string(),
        "MilliSatoshis".to_string(),
        "Msat".to_string(),
        "Amount".to_string(),
        "Bytes".to_string(),
    ];
    assert!(registry.validate_types(known_aliases).is_ok());

    let canonical_types = vec![
        "TransactionInput".to_string(),
        "TransactionOutput".to_string(),
        "Hash".to_string(),
        "bitcoin::Txid".to_string(),
        "bitcoin::BlockHash".to_string(),
        "bitcoin::Amount".to_string(),
        "bitcoin::Address".to_string(),
        "HashOrHeight".to_string(),
        "Vec<bitcoin::Txid>".to_string(),
        "Vec<String>".to_string(),
        "serde_json::Map<String, serde_json::Value>".to_string(),
        "Vec<serde_json::Map<String, serde_json::Value>>".to_string(),
        "PublicKey".to_string(),
        "ShortChannelId".to_string(),
        "u64".to_string(),
        "Vec<u8>".to_string(),
    ];
    assert!(registry.validate_types(canonical_types).is_ok());

    let mixed_types =
        vec!["TxInput".to_string(), "TransactionOutput".to_string(), "Hash".to_string()];
    assert!(registry.validate_types(mixed_types).is_ok());

    let unknown_types = vec!["UnknownType".to_string(), "AnotherUnknown".to_string()];
    let result = registry.validate_types(unknown_types);
    assert!(result.is_err());
    let unknown_list = result.unwrap_err();
    assert_eq!(unknown_list.len(), 2);
    assert!(unknown_list.contains(&"UnknownType".to_string()));
    assert!(unknown_list.contains(&"AnotherUnknown".to_string()));

    let mixed_unknown = vec!["TxInput".to_string(), "UnknownType".to_string(), "Hash".to_string()];
    let result = registry.validate_types(mixed_unknown);
    assert!(result.is_err());
    let unknown_list = result.unwrap_err();
    assert_eq!(unknown_list.len(), 1);
    assert!(unknown_list.contains(&"UnknownType".to_string()));

    let empty_types: Vec<String> = vec![];
    assert!(registry.validate_types(empty_types).is_ok());

    let definitely_unknown = vec!["DefinitelyUnknownType".to_string()];
    let result = registry.validate_types(definitely_unknown);
    assert!(result.is_err(), "validate_types must return Err for definitely unknown types");

    let error_list = result.unwrap_err();
    assert_eq!(error_list.len(), 1);
    assert!(error_list.contains(&"DefinitelyUnknownType".to_string()));
}

#[test]
fn test_is_canonical_type() {
    let mut map = HashMap::new();
    map.insert("BlockHash".to_string(), "Hash".to_string());
    map.insert("TxId".to_string(), "Hash".to_string());
    map.insert("TxInput".to_string(), "TransactionInput".to_string());
    map.insert("TxOutput".to_string(), "TransactionOutput".to_string());

    let registry = TypeAliasRegistry::new(map);

    assert!(registry.is_canonical_type("TransactionInput"), "TransactionInput should be canonical");
    assert!(
        registry.is_canonical_type("TransactionOutput"),
        "TransactionOutput should be canonical"
    );
    assert!(registry.is_canonical_type("Hash"), "Hash should be canonical");

    assert!(
        !registry.is_canonical_type("BlockHash"),
        "BlockHash should not be canonical (it's an alias)"
    );
    assert!(!registry.is_canonical_type("TxId"), "TxId should not be canonical (it's an alias)");
    assert!(
        !registry.is_canonical_type("TxInput"),
        "TxInput should not be canonical (it's an alias)"
    );
    assert!(
        !registry.is_canonical_type("TxOutput"),
        "TxOutput should not be canonical (it's an alias)"
    );

    assert!(!registry.is_canonical_type("UnknownType"), "UnknownType should not be canonical");
    assert!(!registry.is_canonical_type("SomeOtherType"), "SomeOtherType should not be canonical");

    assert!(registry.is_canonical_type("TransactionInput"), "Canonical type must return true");

    assert!(!registry.is_canonical_type("TxInput"), "Alias must return false");

    assert!(!registry.is_canonical_type("DefinitelyUnknownType"), "Unknown type must return false");
}

#[test]
fn test_insert() {
    let mut registry = TypeAliasRegistry::new(HashMap::new());

    registry.insert("TxInput".to_string(), "TransactionInput".to_string());
    assert_eq!(registry.len(), 1);
    assert!(registry.is_alias("TxInput"));
    assert_eq!(registry.resolve("TxInput"), "TransactionInput");

    registry.insert("TxInput".to_string(), "Input".to_string());
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.resolve("TxInput"), "Input");

    registry.insert("TxOutput".to_string(), "TransactionOutput".to_string());
    registry.insert("BlockHash".to_string(), "Hash".to_string());
    assert_eq!(registry.len(), 3);
    assert!(registry.is_alias("TxOutput"));
    assert!(registry.is_alias("BlockHash"));
}
