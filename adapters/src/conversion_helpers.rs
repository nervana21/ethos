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

/// Sorts protocol definitions by RPC method name
///
/// This ensures consistent ordering of definitions across different code paths.
pub fn sort_definitions_by_name(definitions: &mut Vec<ProtocolDef>) {
    definitions.sort_by(|a, b| {
        let name_a = match a {
            ProtocolDef::RpcMethod(ref rpc) => &rpc.name,
            _ => return std::cmp::Ordering::Equal,
        };
        let name_b = match b {
            ProtocolDef::RpcMethod(ref rpc) => &rpc.name,
            _ => return std::cmp::Ordering::Equal,
        };
        name_a.cmp(name_b)
    });
}
