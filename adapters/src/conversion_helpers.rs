// SPDX-License-Identifier: CC0-1.0

//! Shared utilities for converting protocol schemas into ProtocolIR.

use ir::ProtocolDef;

/// Wallet RPC methods that require private key access
const WALLET_METHODS_REQUIRING_PRIVATE_KEYS: &[&str] = &[
    // Signing
    "signmessage",
    "signmessagewithprivkey",
    "signrawtransaction",
    "signrawtransactionwithwallet",
    // Key export / import
    "dumpprivkey",
    "dumpwallet",
    "importprivkey",
    "importwallet",
    // Wallet encryption / passphrase
    "encryptwallet",
    "walletlock",
    "walletpassphrase",
    "walletpassphrasechange",
];

/// Determines if method requires private key access
///
/// Only wallet-category methods are considered; the method must be in the explicit allowlist.
pub fn determine_requires_private_keys(category: &str, method_name: &str) -> bool {
    if category.to_lowercase() != "wallet" {
        return false;
    }
    let name = method_name.to_lowercase();
    WALLET_METHODS_REQUIRING_PRIVATE_KEYS.contains(&name.as_str())
}

/// Sorts protocol definitions: Type defs first (by type name), then RpcMethod defs (by method name).
///
/// This ensures consistent ordering and keeps canonical type definitions before method definitions.
pub fn sort_definitions_by_name(definitions: &mut Vec<ProtocolDef>) {
    definitions.sort_by(|a, b| {
        let (ord_a, name_a) = match a {
            ProtocolDef::Type(ref ty) => (0u8, ty.name.as_str()),
            ProtocolDef::RpcMethod(ref rpc) => (1, rpc.name.as_str()),
            _ => (2, ""),
        };
        let (ord_b, name_b) = match b {
            ProtocolDef::Type(ref ty) => (0, ty.name.as_str()),
            ProtocolDef::RpcMethod(ref rpc) => (1, rpc.name.as_str()),
            _ => (2, ""),
        };
        (ord_a, name_a).cmp(&(ord_b, name_b))
    });
}
