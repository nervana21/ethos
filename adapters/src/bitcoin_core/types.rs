//! Bitcoin Core-specific types and utilities.
//!
//! This module contains types that are specific to Bitcoin Core's RPC interface.

use ::types::{Argument, MethodResult};
use bitcoin::BlockHash;
use serde::{Deserialize, Serialize};
use serde_json;

/// Bitcoin Core-specific type for representing either a block hash or height.
///
/// This type is commonly used in Bitcoin Core RPC methods where a parameter
/// can accept either a block hash (string) or block height (integer).
/// This is a Core-specific convenience, not a universal Bitcoin protocol concept.
///
/// ## Usage
///
/// ```rust
/// use ethos_adapters::bitcoin_core::types::HashOrHeight;
/// use bitcoin::BlockHash;
///
/// let hash_str = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";
/// let block_hash = hash_str.parse::<BlockHash>().unwrap();
/// let by_hash = HashOrHeight::Hash(block_hash);
/// let by_height = HashOrHeight::Height(0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HashOrHeight {
    /// Bitcoin block hash
    Hash(BlockHash),
    /// Block height as an integer
    Height(i64),
}

impl HashOrHeight {
    /// Parse a JSON value into HashOrHeight
    ///
    /// This helper function converts from the raw JSON values that Bitcoin Core
    /// RPC methods accept into the structured HashOrHeight type.
    ///
    /// ## Arguments
    ///
    /// * `value` - A JSON value that can be either a string (hash) or number (height)
    ///
    /// ## Returns
    ///
    /// Returns `Some(HashOrHeight)` if the value can be parsed, `None` otherwise
    ///
    /// ## Usage
    ///
    /// ```rust
    /// use ethos_adapters::bitcoin_core::types::HashOrHeight;
    /// use serde_json::json;
    ///
    /// let hash_value = json!("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f");
    /// let height_value = json!(123456);
    ///
    /// let hash = HashOrHeight::from_json(&hash_value).unwrap();
    /// let height = HashOrHeight::from_json(&height_value).unwrap();
    /// ```
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        value
            .as_str()
            .and_then(|hash_str| hash_str.parse::<BlockHash>().ok())
            .map(HashOrHeight::Hash)
            .or_else(|| value.as_i64().map(HashOrHeight::Height))
            .or_else(|| value.as_u64().map(|h| HashOrHeight::Height(h as i64)))
    }

    /// Convert HashOrHeight back to a JSON value
    ///
    /// This is useful when you need to pass the value back to a Bitcoin Core RPC call.
    ///
    /// ## Returns
    ///
    /// Returns a JSON value representing this HashOrHeight
    ///
    /// ## Usage
    ///
    /// ```rust
    /// use ethos_adapters::bitcoin_core::types::HashOrHeight;
    /// use bitcoin::BlockHash;
    ///
    /// let hash = HashOrHeight::Hash("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f".parse().unwrap());
    /// let json = hash.to_json();
    /// assert_eq!(json.as_str(), Some("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"));
    /// ```
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            HashOrHeight::Hash(hash) => serde_json::to_value(hash)
                .unwrap_or_else(|_| serde_json::Value::String(hash.to_string())),
            HashOrHeight::Height(height) => serde_json::Value::Number((*height).into()),
        }
    }
}

/// Bitcoin Core RPC types based on their semantic meaning and usage patterns.
/// This enum provides a systematic way to categorize and map Bitcoin Core JSON-RPC types
/// to Bitcoin Core-specific Rust types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitcoinCoreRpcType {
    /// Generic string values
    String,
    /// Boolean true/false values
    Boolean,
    /// Null/empty values
    Null,

    /// Bitcoin block hash (32-byte hash encoded as hex)
    BitcoinBlockHash,
    /// Bitcoin transaction hash/transaction ID (32-byte hash encoded as hex)
    BitcoinTxid,
    /// Bitcoin amounts expressed in satoshis (1 BTC = 100,000,000 satoshis)
    BitcoinAmount,
    /// Bitcoin addresses (P2PKH, P2SH, Bech32, Bech32m, etc.)
    BitcoinAddress,
    /// Bitcoin scripts (generic script buffers)
    BitcoinScript,
    /// Bitcoin script pubkeys (output scripts)
    BitcoinScriptPubKey,

    /// Type that can be either a block hash or block height
    HashOrHeight,
    /// Structured Bitcoin objects (blocks, transactions, UTXOs, etc.)
    BitcoinObject,
    /// Arrays of structured Bitcoin objects
    BitcoinObjectArray,

    /// Arrays of transaction IDs (txids)
    TxidArray,
    /// Arrays of strings (addresses, keys, wallet names, etc.)
    StringArray,

    /// Network port numbers (0-65535)
    Port,
    /// Small bounded integers (u32) for version numbers, verbosity levels, etc.
    SmallInteger,
    /// Large integers (u64) for block heights, timestamps, counts, sizes
    LargeInteger,
    /// Signed integers (i64) that can be negative, e.g., confirmations (-1 for orphaned blocks), changepos (-1 for no change output)
    SignedInteger,
    /// Floating-point values for rates, probabilities, percentages, difficulties
    Float,
    /// Unix epoch timestamps (u64)
    Timestamp,

    /// No return value (void/unit type)
    None,
    /// Echo/passthrough type for arbitrary data
    Any,
    /// Numeric range parameters
    Range,
    /// Documentation placeholder
    Elision,

    /// Optional dummy fields for testing
    Dummy,
}

impl BitcoinCoreRpcType {
    /// Convert the category to its corresponding Rust type string
    pub fn to_rust_type(&self) -> &'static str {
        match self {
            BitcoinCoreRpcType::String => "String",
            BitcoinCoreRpcType::Boolean => "bool",
            BitcoinCoreRpcType::Null => "()",
            BitcoinCoreRpcType::BitcoinBlockHash => "bitcoin::BlockHash",
            BitcoinCoreRpcType::BitcoinTxid => "bitcoin::Txid",
            BitcoinCoreRpcType::BitcoinAmount => "bitcoin::Amount",
            BitcoinCoreRpcType::BitcoinAddress => "bitcoin::Address",
            BitcoinCoreRpcType::BitcoinScript => "bitcoin::ScriptBuf",
            BitcoinCoreRpcType::BitcoinScriptPubKey => "bitcoin::ScriptBuf",
            BitcoinCoreRpcType::HashOrHeight => "HashOrHeight",
            BitcoinCoreRpcType::BitcoinObject => "serde_json::Map<String, serde_json::Value>",
            BitcoinCoreRpcType::BitcoinObjectArray =>
                "Vec<serde_json::Map<String, serde_json::Value>>",
            BitcoinCoreRpcType::TxidArray => "Vec<bitcoin::Txid>",
            BitcoinCoreRpcType::StringArray => "Vec<String>",
            BitcoinCoreRpcType::Port => "u16",
            BitcoinCoreRpcType::SmallInteger => "u32",
            BitcoinCoreRpcType::LargeInteger => "u64",
            BitcoinCoreRpcType::SignedInteger => "i64",
            BitcoinCoreRpcType::Float => "f64",
            BitcoinCoreRpcType::Timestamp => "u64",
            BitcoinCoreRpcType::None => "()",
            BitcoinCoreRpcType::Any => "serde_json::Value",
            BitcoinCoreRpcType::Elision => "serde_json::Value",
            BitcoinCoreRpcType::Range => "serde_json::Value",
            BitcoinCoreRpcType::Dummy => "String",
        }
    }

    /// Check if this category should be treated as optional by default
    pub fn is_optional_by_default(&self) -> bool { matches!(self, Self::Dummy) }

    /// Returns the serde attribute for converting Bitcoin RPC float amounts to satoshis
    ///
    /// Bitcoin RPC returns amounts as BTC floats, but we need satoshis for type safety.
    /// This provides the deserialization attribute to handle the conversion automatically.
    pub fn bitcoin_amount_deserializer_attribute(&self) -> Option<&'static str> {
        match self {
            BitcoinCoreRpcType::BitcoinAmount =>
                Some("#[serde(deserialize_with = \"amount_from_btc_float\")]"),
            _ => None,
        }
    }

    /// Get a description of this category for documentation
    pub fn description(&self) -> &'static str {
        match self {
			BitcoinCoreRpcType::String => "Generic string values",
			BitcoinCoreRpcType::Boolean => "Boolean true/false values",
			BitcoinCoreRpcType::Null => "Null/empty values",
			BitcoinCoreRpcType::BitcoinBlockHash => {
				"Bitcoin block hash (32-byte hash encoded as hex)"
			},
			BitcoinCoreRpcType::BitcoinTxid => {
				"Bitcoin transaction hash/transaction ID (32-byte hash encoded as hex)"
			},
			BitcoinCoreRpcType::BitcoinAmount => {
				"Bitcoin amounts expressed in satoshis (1 BTC = 100,000,000 satoshis)"
			},
			BitcoinCoreRpcType::BitcoinAddress => {
				"Bitcoin addresses (P2PKH, P2SH, Bech32, Bech32m, etc.)"
			},
			BitcoinCoreRpcType::BitcoinScript => {
				"Bitcoin scripts (hex-encoded script buffers)"
			},
			BitcoinCoreRpcType::BitcoinScriptPubKey => {
				"Bitcoin script pubkeys (hex-encoded output scripts)"
			},
			BitcoinCoreRpcType::HashOrHeight => {
				"Type that can be either a block hash or block height"
			},
			BitcoinCoreRpcType::BitcoinObject => {
				"Structured Bitcoin objects (blocks, transactions, UTXOs, etc.)"
			},
			BitcoinCoreRpcType::BitcoinObjectArray => "Arrays of structured Bitcoin objects",
			BitcoinCoreRpcType::TxidArray => "Arrays of transaction IDs (txids)",
			BitcoinCoreRpcType::StringArray => {
				"Arrays of strings (addresses, keys, wallet names, etc.)"
			},
			BitcoinCoreRpcType::Port => "Network port numbers (0-65535)",
			BitcoinCoreRpcType::SmallInteger => {
				"Small bounded integers (u32) for version numbers, verbosity levels, etc."
			},
			BitcoinCoreRpcType::LargeInteger => {
				"Large integers (u64) for block heights, timestamps, counts, sizes"
			},
			BitcoinCoreRpcType::SignedInteger => {
				"Signed integers (i64) that can be negative, e.g., confirmations (-1 for orphaned blocks), changepos (-1 for no change output)"
			},
			BitcoinCoreRpcType::Float => {
				"Floating-point values for rates, probabilities, percentages, difficulties"
			},
			BitcoinCoreRpcType::Timestamp => "Unix epoch timestamps (u64)",
			BitcoinCoreRpcType::None => "No return value (void/unit type)",
			BitcoinCoreRpcType::Any => "Echo/passthrough type for arbitrary data",
			BitcoinCoreRpcType::Range => "Numeric range parameters",
			BitcoinCoreRpcType::Elision => "Documentation placeholder",
			BitcoinCoreRpcType::Dummy => "Optional dummy fields for testing",
		}
    }
}

/// A registry of type mappings for Bitcoin Core JSON RPC types to Rust types.
///
/// Registry for mapping Bitcoin Core JSON RPC type identifiers to Rust types.
/// Handles optional fields and supports wildcard matching for dynamic types.
pub struct BitcoinCoreTypeRegistry;

impl BitcoinCoreTypeRegistry {
    /// Categorize an RPC type based on its JSON schema type and field name
    fn categorize(rpc_type: &str, field: &str) -> BitcoinCoreRpcType {
        let field_norm = normalize(field);
        let rpc_json_type = RpcJsonType::from_str(rpc_type);

        let mut catchall: Option<BitcoinCoreRpcType> = None;

        for rule in CATEGORY_RULES {
            if rule.rpc_type == rpc_json_type {
                match rule.field_name {
                    // Specific field name match - return immediately
                    Some(rule_field) if field_norm == normalize(rule_field) => {
                        return rule.category;
                    }
                    // Catchall rule - store for fallback
                    None => {
                        catchall = Some(rule.category);
                    }
                    _ => continue,
                }
            }
        }

        // Return catchall if available, otherwise panic
        catchall.unwrap_or_else(|| {
            panic!(
                "No CATEGORY_RULES match for rpc_type='{}' field='{}'. Add an explicit rule.",
                rpc_type, field
            )
        })
    }

    /// Categorizes the Argument type
    pub fn categorize_argument(arg: &Argument) -> BitcoinCoreRpcType {
        Self::categorize(&arg.type_, &arg.names[0])
    }

    /// Categorizes the MethodResult type
    fn categorize_result(result: &MethodResult) -> BitcoinCoreRpcType {
        let name = &result.key_name;
        Self::categorize(&result.type_, name)
    }

    /// Core mapper that returns the Rust type and whether the field is optional
    pub fn map(rpc_type: &str, field: &str) -> (&'static str, bool) {
        let category = Self::categorize(rpc_type, field);
        (category.to_rust_type(), false)
    }

    /// Maps the Argument type to the Rust type and whether the field is optional
    pub fn map_argument_type(arg: &Argument) -> (&'static str, bool) {
        let field = &arg.names[0];
        let (ty, _) = Self::map(&arg.type_, field);
        (ty, !arg.required)
    }

    /// Maps the MethodResult type to the Rust type and whether the field is optional
    pub fn map_result_type(result: &MethodResult) -> (&'static str, bool) {
        let category = Self::categorize_result(result);
        let ty = category.to_rust_type();
        (ty, result.optional)
    }
}

/// Normalizes names by lowercasing and stripping `_`, `-`, and spaces.
fn normalize(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '_' | '-' | ' ')).flat_map(|c| c.to_lowercase()).collect()
}

/// Internal JSON schema-like primitive classifier
///
/// Used by `BitcoinCoreTypeRegistry` to categorize raw RPC schema types
/// before mapping them to `BitcoinCoreRpcType`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RpcJsonType {
    /// JSON strings
    String,
    /// JSON numbers (integers or floats)
    Number,
    /// JSON booleans
    Boolean,
    /// JSON null values
    Null,
    /// Wildcard `*` from schemas (accepts any)
    Wildcard,
    /// Bitcoin Core amount domain from IR/schemas
    Amount,
    /// Hex-encoded strings
    Hex,
    /// JSON arrays
    Array,
    /// JSON objects
    Object,
    /// Unix epoch timestamps (u64)
    Timestamp,
    /// No value sentinel (void/unit)
    NoneType,
    /// Arbitrary JSON values
    Any,
    /// Documentation placeholder
    Elision,
    /// Numeric range parameters
    Range,
}

impl RpcJsonType {
    fn from_str(s: &str) -> Self {
        match s {
            "string" => Self::String,
            "number" => Self::Number,
            "boolean" => Self::Boolean,
            "null" => Self::Null,
            "*" => Self::Wildcard,
            "amount" => Self::Amount,
            "hex" => Self::Hex,
            "array" => Self::Array,
            "object" => Self::Object,
            "timestamp" => Self::Timestamp,
            "none" => Self::NoneType,
            "any" => Self::Any,
            "elision" => Self::Elision,
            "range" => Self::Range,
            _ => Self::Any,
        }
    }
}

/// A mapping rule that categorizes RPC types based on their JSON schema type and field name patterns
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct CategoryRule {
    /// JSON Schema type of the field (e.g., "string").
    rpc_type: RpcJsonType,
    /// Optional normalized field name hint to refine categorization.
    field_name: Option<&'static str>,
    /// Resolved internal category driving downstream type mapping.
    category: BitcoinCoreRpcType,
}

#[rustfmt::skip]
const CATEGORY_RULES: &[CategoryRule] = &[
    // Primitives (no pattern needed)
    CategoryRule {
        rpc_type: RpcJsonType::String,
        field_name: None,
        category: BitcoinCoreRpcType::String,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Boolean,
        field_name: None,
        category: BitcoinCoreRpcType::Boolean,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Null,
        field_name: None,
        category: BitcoinCoreRpcType::Null,
    },
    // Bitcoin Core-specific string types
    CategoryRule {
        rpc_type: RpcJsonType::String,
        field_name: Some("txid"),
        category: BitcoinCoreRpcType::BitcoinTxid,
    },
    CategoryRule {
        rpc_type: RpcJsonType::String,
        field_name: Some("blockhash"),
        category: BitcoinCoreRpcType::BitcoinBlockHash,
    },
    // Script types
    CategoryRule {
        rpc_type: RpcJsonType::String,
        field_name: Some("scriptpubkey"),
        category: BitcoinCoreRpcType::BitcoinScriptPubKey,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Hex,
        field_name: Some("scriptpubkey"),
        category: BitcoinCoreRpcType::BitcoinScriptPubKey,
    },
    CategoryRule {
        rpc_type: RpcJsonType::String,
        field_name: Some("script"),
        category: BitcoinCoreRpcType::BitcoinScript,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Hex,
        field_name: Some("script"),
        category: BitcoinCoreRpcType::BitcoinScript,
    },
    CategoryRule {
        rpc_type: RpcJsonType::String,
        field_name: Some("redeemscript"),
        category: BitcoinCoreRpcType::BitcoinScript,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Hex,
        field_name: Some("redeemscript"),
        category: BitcoinCoreRpcType::BitcoinScript,
    },
    CategoryRule {
        rpc_type: RpcJsonType::String,
        field_name: Some("witnessscript"),
        category: BitcoinCoreRpcType::BitcoinScript,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Hex,
        field_name: Some("witnessscript"),
        category: BitcoinCoreRpcType::BitcoinScript,
    },
    // Hash or height union type
    CategoryRule {
        rpc_type: RpcJsonType::String,
        field_name: Some("hash_or_height"),
        category: BitcoinCoreRpcType::HashOrHeight,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("hash_or_height"),
        category: BitcoinCoreRpcType::HashOrHeight,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Wildcard,
        field_name: Some("hash_or_height"),
        category: BitcoinCoreRpcType::HashOrHeight,
    },
    // Bitcoin Core amounts (monetary values)
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("amount"),
        category: BitcoinCoreRpcType::BitcoinAmount,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("balance"),
        category: BitcoinCoreRpcType::BitcoinAmount,
    },
    // Handle "type": "amount" fields - specific patterns first
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("balance"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("fee_rate"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("estimated_feerate"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("maxfeerate"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("maxburnamount"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("relayfee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("incrementalfee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("incrementalrelayfee"),
        category: BitcoinCoreRpcType::Float,
    },
    // Amount fields that Bitcoin Core returns as BTC floats
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("mempoolminfee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("minrelaytxfee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("total_fee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: Some("blockmintxfee"),
        category: BitcoinCoreRpcType::Float,
    },
    // Default rule for "type": "amount" fields that don't match specific patterns (like result amounts)
    CategoryRule {
        rpc_type: RpcJsonType::Amount,
        field_name: None,
        category: BitcoinCoreRpcType::BitcoinAmount,
    },
    // Fee and rate fields (floating point)
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("fee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("rate"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("feerate"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("maxfeerate"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("maxburnamount"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("relayfee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("incrementalfee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("incrementalrelayfee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("mempoolminfee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("minrelaytxfee"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("difficulty"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("probability"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("percentage"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("fee_rate"),
        category: BitcoinCoreRpcType::Float,
    },
    // Port numbers
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("port"),
        category: BitcoinCoreRpcType::Port,
    },
    // Small integers (u32)
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("nrequired"),
        category: BitcoinCoreRpcType::SmallInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("minconf"),
        category: BitcoinCoreRpcType::SmallInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("maxconf"),
        category: BitcoinCoreRpcType::SmallInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("locktime"),
        category: BitcoinCoreRpcType::SmallInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("version"),
        category: BitcoinCoreRpcType::SmallInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("verbosity"),
        category: BitcoinCoreRpcType::SmallInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("checklevel"),
        category: BitcoinCoreRpcType::SmallInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("n"),
        category: BitcoinCoreRpcType::SmallInteger,
    },
    // Large integers (u64)
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("blocks"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("maxtries"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("height"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("count"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("index"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("size"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("time"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("conf_target"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("skip"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("nodeid"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("peer_id"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("wait"),
        category: BitcoinCoreRpcType::LargeInteger,
    },
    // Signed integers (can be negative)
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("changepos"),
        category: BitcoinCoreRpcType::SignedInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("confirmations"),
        category: BitcoinCoreRpcType::SignedInteger,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("nblocks"),
        category: BitcoinCoreRpcType::SignedInteger,
    },
    // Hex types
    CategoryRule {
        rpc_type: RpcJsonType::Hex,
        field_name: Some("txid"),
        category: BitcoinCoreRpcType::BitcoinTxid,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Hex,
        field_name: Some("blockhash"),
        category: BitcoinCoreRpcType::BitcoinBlockHash,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Hex,
        field_name: None,
        category: BitcoinCoreRpcType::String,
    },
    // Array types
    CategoryRule {
        rpc_type: RpcJsonType::Array,
        field_name: Some("keys"),
        category: BitcoinCoreRpcType::StringArray,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Array,
        field_name: Some("addresses"),
        category: BitcoinCoreRpcType::StringArray,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Array,
        field_name: Some("wallets"),
        category: BitcoinCoreRpcType::StringArray,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Array,
        field_name: Some("stats"),
        category: BitcoinCoreRpcType::StringArray,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Array,
        field_name: Some("txids"),
        category: BitcoinCoreRpcType::TxidArray,
    },
    // Default rule for generic arrays that don't match specific patterns
    CategoryRule {
        rpc_type: RpcJsonType::Array,
        field_name: None,
        category: BitcoinCoreRpcType::StringArray,
    },
    // Object types - specific patterns first
    CategoryRule {
        rpc_type: RpcJsonType::Object,
        field_name: Some("options"),
        category: BitcoinCoreRpcType::BitcoinObject,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Object,
        field_name: Some("query_options"),
        category: BitcoinCoreRpcType::BitcoinObject,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Object,
        field_name: None,
        category: BitcoinCoreRpcType::BitcoinObject,
    },
    // Specific floating-point fields that should be f64
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("verificationprogress"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("difficulty"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("networkhashps"),
        category: BitcoinCoreRpcType::Float,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("incrementalrelayfee"),
        category: BitcoinCoreRpcType::Float,
    },
    // Catchall for remaining number fields
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: None,
        category: BitcoinCoreRpcType::LargeInteger,
    },
    // New Bitcoin Core types
    CategoryRule {
        rpc_type: RpcJsonType::Timestamp,
        field_name: None,
        category: BitcoinCoreRpcType::Timestamp,
    },
    CategoryRule {
        rpc_type: RpcJsonType::NoneType,
        field_name: None,
        category: BitcoinCoreRpcType::None,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Any,
        field_name: None,
        category: BitcoinCoreRpcType::Any,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Elision,
        field_name: None,
        category: BitcoinCoreRpcType::Elision,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Range,
        field_name: None,
        category: BitcoinCoreRpcType::Range,
    },
    // Dummy fields (for testing)
    CategoryRule {
        rpc_type: RpcJsonType::String,
        field_name: Some("dummy"),
        category: BitcoinCoreRpcType::Dummy,
    },
    CategoryRule {
        rpc_type: RpcJsonType::Number,
        field_name: Some("dummy"),
        category: BitcoinCoreRpcType::Dummy,
    },
];
