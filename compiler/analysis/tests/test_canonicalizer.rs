use ethos_analysis::TypeCanonicalizer;
use ir::{ProtocolDef, ProtocolIR, ProtocolModule, TypeDef, TypeKind};

#[test]
fn test_canonicalize() {
    // Create a ProtocolIR with multiple type definitions
    let mut ir = ProtocolIR::new(vec![ProtocolModule::new(
        "rpc".to_string(),
        "RPC Module".to_string(),
        vec![
            ProtocolDef::Type(TypeDef {
                name: "Address".to_string(),
                description: "Bitcoin address".to_string(),
                kind: TypeKind::Primitive,
                fields: None,
                variants: None,
                base_type: None,
                protocol_type: None,
                canonical_name: None,
                condition: None,
            }),
            ProtocolDef::Type(TypeDef {
                name: "BitcoinAddress".to_string(),
                description: "Bitcoin address type".to_string(),
                kind: TypeKind::Primitive,
                fields: None,
                variants: None,
                base_type: None,
                protocol_type: None,
                canonical_name: None,
                condition: None,
            }),
            ProtocolDef::Type(TypeDef {
                name: "WalletAddress".to_string(),
                description: "Wallet address".to_string(),
                kind: TypeKind::Primitive,
                fields: None,
                variants: None,
                base_type: None,
                protocol_type: None,
                canonical_name: None,
                condition: None,
            }),
            ProtocolDef::Type(TypeDef {
                name: "BlockHash".to_string(),
                description: "Block hash".to_string(),
                kind: TypeKind::Object,
                fields: None,
                variants: None,
                base_type: None,
                protocol_type: None,
                canonical_name: None,
                condition: None,
            }),
        ],
    )]);

    let canonicalizer = TypeCanonicalizer;
    let mapping = canonicalizer.canonicalize(&mut ir);

    assert_eq!(mapping.len(), 2);
    // Address is the canonical type, so it shouldn't be in the mapping
    assert_eq!(mapping.get("BitcoinAddress"), Some(&"Address".to_string()));
    assert_eq!(mapping.get("WalletAddress"), Some(&"Address".to_string()));

    let module = &ir.modules()[0];
    let definitions = module.definitions();

    let mut address_type = None;
    let mut bitcoin_address_type = None;
    let mut wallet_address_type = None;
    let mut block_hash_type = None;

    for def in definitions {
        if let ProtocolDef::Type(ty) = def {
            match ty.name.as_str() {
                "Address" => address_type = Some(ty),
                "BitcoinAddress" => bitcoin_address_type = Some(ty),
                "WalletAddress" => wallet_address_type = Some(ty),
                "BlockHash" => block_hash_type = Some(ty),
                _ => {}
            }
        }
    }

    let address = address_type.expect("Address type should exist");
    assert_eq!(address.canonical_name, None);

    let bitcoin_address = bitcoin_address_type.expect("BitcoinAddress type should exist");
    assert_eq!(bitcoin_address.canonical_name, Some("Address".to_string()));

    let wallet_address = wallet_address_type.expect("WalletAddress type should exist");
    assert_eq!(wallet_address.canonical_name, Some("Address".to_string()));

    let block_hash = block_hash_type.expect("BlockHash type should exist");
    assert_eq!(block_hash.canonical_name, None);
}
