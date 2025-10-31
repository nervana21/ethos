//! Method categorization for feature-based organization
//!
//! This module categorizes RPC methods into semantic groups for modular
//! codegen, feature gating, and cross-backend normalization.

use ir::{AccessLevel, RpcDef};

/// Categories for RPC methods based on Bitcoin ecosystem architecture
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MethodCategory {
    /// Blockchain/chain state methods — durable consensus state (headers, UTXO, validation)
    Blockchain,
    /// Control methods (help, stop, etc.)
    Control,
    /// Generating methods (block generation, depends on mining)
    Generating,
    /// Mempool methods — ephemeral relay/policy surface; may be disabled or absent.
    /// Note: some projects group mempool with blockchain; here they are separate to reflect
    /// distinct operational domains and enable independent feature gating.
    Mempool,
    /// Mining methods
    Mining,
    /// Network/P2P methods
    Network,
    /// Raw transaction methods (pure functions)
    Rawtransaction,
    /// Utility methods
    Util,
    /// Wallet methods (optional subsystem)
    Wallet,
    /// ZMQ methods
    Zmq,
    /// Signer methods (hardware signing operations)
    Signer,
    /// Lightning channel operations
    Channel,
    /// Lightning payment operations
    Payment,
    /// Lightning invoice operations
    Invoice,
    /// Query operations (read-only data access)
    Query,
    /// Create operations (resource creation)
    Create,
    /// Delete operations (resource deletion)
    Delete,
    /// Core operations (fundamental protocol operations)
    Core,
}

impl MethodCategory {
    /// Canonical display name used for sorting and headings
    pub fn display_name(&self) -> &'static str {
        match self {
            MethodCategory::Blockchain => "blockchain",
            MethodCategory::Control => "control",
            MethodCategory::Generating => "generating",
            MethodCategory::Mempool => "mempool",
            MethodCategory::Mining => "mining",
            MethodCategory::Network => "network",
            MethodCategory::Rawtransaction => "rawtransactions",
            MethodCategory::Util => "util",
            MethodCategory::Wallet => "wallet",
            MethodCategory::Zmq => "zmq",
            MethodCategory::Signer => "signer",
            MethodCategory::Channel => "channel",
            MethodCategory::Payment => "payment",
            MethodCategory::Invoice => "invoice",
            MethodCategory::Query => "query",
            MethodCategory::Create => "create",
            MethodCategory::Delete => "delete",
            MethodCategory::Core => "core",
        }
    }

    /// Get the feature flag name for this category
    pub fn feature_name(&self) -> &'static str {
        match self {
            MethodCategory::Blockchain => "blockchain",
            MethodCategory::Wallet => "wallet",
            MethodCategory::Network => "network",
            MethodCategory::Mining => "mining",
            MethodCategory::Mempool => "mempool",
            MethodCategory::Rawtransaction => "rawtransaction",
            MethodCategory::Util => "util",
            MethodCategory::Control => "control",
            MethodCategory::Generating => "generating",
            MethodCategory::Zmq => "zmq",
            MethodCategory::Signer => "signer",
            MethodCategory::Channel => "channel",
            MethodCategory::Payment => "payment",
            MethodCategory::Invoice => "invoice",
            MethodCategory::Query => "query",
            MethodCategory::Create => "create",
            MethodCategory::Delete => "delete",
            MethodCategory::Core => "core",
        }
    }

    /// Get the directory name for this category
    pub fn dir_name(&self) -> &'static str {
        match self {
            MethodCategory::Blockchain => "blockchain",
            MethodCategory::Wallet => "wallet",
            MethodCategory::Network => "network",
            MethodCategory::Mining => "mining",
            MethodCategory::Mempool => "mempool",
            MethodCategory::Rawtransaction => "rawtransaction",
            MethodCategory::Util => "util",
            MethodCategory::Control => "control",
            MethodCategory::Generating => "generating",
            MethodCategory::Zmq => "zmq",
            MethodCategory::Signer => "signer",
            MethodCategory::Channel => "channel",
            MethodCategory::Payment => "payment",
            MethodCategory::Invoice => "invoice",
            MethodCategory::Query => "query",
            MethodCategory::Create => "create",
            MethodCategory::Delete => "delete",
            MethodCategory::Core => "core",
        }
    }

    /// Check if this category should be included by default
    pub fn is_default(&self) -> bool {
        matches!(
            self,
            MethodCategory::Blockchain
                | MethodCategory::Network
                | MethodCategory::Util
                | MethodCategory::Rawtransaction
                | MethodCategory::Query
                | MethodCategory::Core
        )
    }
}

/// Map protocol-specific category labels to the unified MethodCategory enum
fn map_protocol_category_label_to_method_category(
    protocol_category: &str,
    method_name: &str,
) -> MethodCategory {
    match protocol_category.to_lowercase().as_str() {
        // Bitcoin
        "blockchain" => MethodCategory::Blockchain,
        "wallet" => MethodCategory::Wallet,
        "network" => MethodCategory::Network,
        "mining" => MethodCategory::Mining,
        "mempool" => MethodCategory::Mempool,
        "rawtransactions" => MethodCategory::Rawtransaction,
        "util" => MethodCategory::Util,
        "control" => MethodCategory::Control,
        "generating" => MethodCategory::Generating,
        "zmq" => MethodCategory::Zmq,
        "signer" => MethodCategory::Signer,
        // Upstream sometimes tags special-case methods as "hidden".
        // Map them to a stable bucket for grouping; access level is handled separately.
        "hidden" => MethodCategory::Core,

        // Lightning
        "channel" => MethodCategory::Channel,
        "payment" => MethodCategory::Payment,
        "invoice" => MethodCategory::Invoice,
        "query" => MethodCategory::Query,
        "core" => MethodCategory::Core,
        "create" => MethodCategory::Create,
        "delete" => MethodCategory::Delete,

        _ => {
            panic!("Unknown protocol category '{}' for method '{}'", protocol_category, method_name)
        }
    }
}

/// Get method access level from protocol category
pub fn access_level_for(category: &str, method_name: &str) -> AccessLevel {
    if category.to_lowercase() == "hidden" {
        let name_lower = method_name.to_lowercase();

        // Testing/regtest methods
        if name_lower.starts_with("generate")
            || name_lower.starts_with("mock")
            || name_lower == "setmocktime"
        {
            return AccessLevel::Testing;
        }

        // Internal/debugging
        if name_lower.starts_with("getorphan")
            || name_lower.starts_with("getrawaddrman")
            || name_lower.starts_with("echo")
            || name_lower == "sendmsgtopeer"
        {
            return AccessLevel::Internal;
        }

        // Advanced/dangerous operations
        if name_lower == "invalidateblock"
            || name_lower == "reconsiderblock"
            || name_lower.starts_with("addconnection")
            || name_lower.starts_with("addpeeraddress")
        {
            return AccessLevel::Advanced;
        }
    }

    AccessLevel::Public
}

/// Categorize method based on existing category or name heuristics
pub fn categorize_method(method: &RpcDef) -> MethodCategory {
    let category_str = &method.category;

    if category_str.trim().is_empty() {
        panic!("categorize_method: Category is empty for method: {}", method.name);
    }

    map_protocol_category_label_to_method_category(category_str, &method.name)
}

/// Group methods by semantic category
pub fn group_methods_by_category(
    methods: &[RpcDef],
) -> std::collections::HashMap<MethodCategory, Vec<&RpcDef>> {
    let mut groups = std::collections::HashMap::new();

    for method in methods {
        let category = categorize_method(method);
        groups.entry(category).or_insert_with(Vec::new).push(method);
    }

    groups
}
