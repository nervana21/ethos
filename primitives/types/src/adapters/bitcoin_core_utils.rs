//! Shared Bitcoin Core type mapping utilities
//!
//! This module provides shared utilities for Bitcoin Core type mapping that are used
//! by both the `TypeAdapter` implementation in this crate and the `BitcoinCoreTypeRegistry`
//! in the `adapters` crate.

/// Normalize field name for matching (lowercase, strip special characters)
///
/// This function normalizes field names by:
/// - Converting to lowercase
/// - Removing underscores, hyphens, and spaces
///
/// This normalization is used to match field names against categorization rules,
/// allowing rules to match regardless of naming conventions (e.g., "block_hash",
/// "block-hash", "blockhash" all match).
///
/// # Examples
///
/// ```
/// use types::adapters::bitcoin_core_utils::normalize_field_name;
///
/// assert_eq!(normalize_field_name("block_hash"), "blockhash");
/// assert_eq!(normalize_field_name("block-hash"), "blockhash");
/// assert_eq!(normalize_field_name("BlockHash"), "blockhash");
/// assert_eq!(normalize_field_name("tx_id"), "txid");
/// ```
pub fn normalize_field_name(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '_' | '-' | ' ')).flat_map(|c| c.to_lowercase()).collect()
}

/// Map Bitcoin Core parameter type to Rust type based on type and field name
///
/// This function implements the parameter-specific type mapping rules for Bitcoin Core.
/// It handles the conversion from Bitcoin Core RPC parameter types to appropriate Rust types.
///
/// The mapping rules prioritize specific field-name matches (e.g., "blockhash" → `bitcoin::BlockHash`)
/// over generic type mappings (e.g., "string" → `String`).
///
/// # Arguments
///
/// * `param_type` - The parameter type name (e.g., "string", "number", "hex")
/// * `param_name` - The parameter name for context-specific mapping
///
/// # Returns
///
/// Rust type as a string (e.g., "String", "i64", "bitcoin::BlockHash")
pub fn map_parameter_type_to_rust(param_type: &str, param_name: &str) -> String {
    let normalized_param = normalize_field_name(param_name);

    if normalized_param == "hashorheight" {
        return "HashOrHeight".to_owned();
    }

    if matches!(param_type, "string" | "hex") {
        // Specific field-name rules for strongly-typed Bitcoin types
        // Fall back to String for generic string/hex parameters
        return match normalized_param.as_str() {
            "address" => "bitcoin::Address",
            "blockhash" => "bitcoin::BlockHash",
            "txid" => "bitcoin::Txid",
            "scriptpubkey" | "script" | "redeemscript" | "witnessscript" => "bitcoin::ScriptBuf",
            _ => "String",
        }
        .to_owned();
    }

    // Amount-domain parameters get more specific handling based on field name.
    if param_type == "amount" {
        // feerate: sat/vB, maxfeerate: BTC/kvB. Both are fee *rates*, not plain amounts.
        // Represent them with the shared FeeRate type; other amount fields stay as bitcoin::Amount.
        return match normalized_param.as_str() {
            "feerate" | "maxfeerate" => "FeeRate".to_owned(),
            _ => "bitcoin::Amount".to_owned(),
        };
    }

    // Object params with known shapes
    if param_type == "object" {
        return match normalized_param.as_str() {
            // sendmany: address -> amount (BTC in JSON; rust-bitcoin Amount in Rust)
            "amounts" => "std::collections::HashMap<bitcoin::Address<bitcoin::address::NetworkUnchecked>, bitcoin::Amount>".to_owned(),
            // getblocktemplate: mode, capabilities, rules (struct emitted in generated params)
            "templaterequest" => "GetBlockTemplateRequest".to_owned(),
            _ => "serde_json::Value".to_owned(),
        };
    }

    // Array params with known element types
    if param_type == "array" {
        return match normalized_param.as_str() {
            // sendmany: list of address strings to subtract fee from
            "subtractfeefrom" =>
                "Vec<bitcoin::Address<bitcoin::address::NetworkUnchecked>>".to_owned(),
            // sendall: list of { address, amount? } (struct emitted in generated params)
            "recipients" => "Vec<SendallRecipient>".to_owned(),
            _ => "Vec<serde_json::Value>".to_owned(),
        };
    }

    match param_type {
        // All numbers are i64 by default (including signed integers that can be negative)
        // Specific field names like "changepos", "confirmations", "nblocks" can accept -1
        "number" | "int" | "integer" => "i64",
        "boolean" | "bool" => "bool",
        _ => "serde_json::Value",
    }
    .to_owned()
}

/// Map a Bitcoin Core MethodResult-like triple to a Rust type string.
///
/// Shared by `TypeAdapter` and `BitcoinCoreTypeRegistry`.
pub fn map_result_type_to_rust(type_: &str, key_name: &str, description: &str) -> &'static str {
    if type_ == "number" && key_name.is_empty() && description.contains("difficulty") {
        return "f64";
    }

    let category = categorize_result_field(type_, key_name);
    result_category_to_rust_type(category)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ResultRpcJsonType {
    String,
    Number,
    Boolean,
    Null,
    Wildcard,
    Amount,
    Hex,
    Array,
    Object,
    Timestamp,
    NoneType,
    Any,
    Elision,
    Range,
}

impl ResultRpcJsonType {
    fn from_str(s: &str) -> Self {
        match s {
            "string" => Self::String,
            "number" => Self::Number,
            "boolean" => Self::Boolean,
            "null" => Self::Null,
            "amount" => Self::Amount,
            "hex" => Self::Hex,
            "array" | "array-fixed" => Self::Array,
            "object" | "object-dynamic" | "object-one-of" => Self::Object,
            "string-or-string-array" => Self::String,
            "bool-or-object" => Self::Boolean,
            "timestamp" => Self::Timestamp,
            "none" => Self::NoneType,
            "any" => Self::Any,
            "elision" => Self::Elision,
            "range" => Self::Range,
            _ => Self::Any,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ResultCategory {
    String,
    Boolean,
    Null,
    BitcoinBlockHash,
    BitcoinTxid,
    BitcoinAmount,
    BitcoinScript,
    BitcoinScriptPubKey,
    HashOrHeight,
    BitcoinObject,
    TxidArray,
    StringArray,
    Port,
    SmallInteger,
    LargeInteger,
    SignedInteger,
    Float,
    Timestamp,
    None,
    Any,
    Range,
    Elision,
    Dummy,
}

fn result_category_to_rust_type(category: ResultCategory) -> &'static str {
    match category {
        ResultCategory::String => "String",
        ResultCategory::Boolean => "bool",
        ResultCategory::Null => "()",
        ResultCategory::BitcoinBlockHash => "bitcoin::BlockHash",
        ResultCategory::BitcoinTxid => "bitcoin::Txid",
        ResultCategory::BitcoinAmount => "bitcoin::Amount",
        ResultCategory::BitcoinScript | ResultCategory::BitcoinScriptPubKey => "bitcoin::ScriptBuf",
        ResultCategory::HashOrHeight => "HashOrHeight",
        ResultCategory::BitcoinObject => "serde_json::Map<String, serde_json::Value>",
        ResultCategory::TxidArray => "Vec<bitcoin::Txid>",
        ResultCategory::StringArray => "Vec<String>",
        ResultCategory::Port => "u16",
        ResultCategory::SmallInteger => "u32",
        ResultCategory::LargeInteger => "u64",
        ResultCategory::SignedInteger => "i64",
        ResultCategory::Float => "f64",
        ResultCategory::Timestamp => "u64",
        ResultCategory::None => "()",
        ResultCategory::Any | ResultCategory::Elision | ResultCategory::Range =>
            "serde_json::Value",
        ResultCategory::Dummy => "String",
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ResultCategoryRule {
    rpc_type: ResultRpcJsonType,
    field_name: Option<&'static str>,
    category: ResultCategory,
}

#[rustfmt::skip]
const RESULT_CATEGORY_RULES: &[ResultCategoryRule] = &[
    ResultCategoryRule { rpc_type: ResultRpcJsonType::String, field_name: None, category: ResultCategory::String },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Boolean, field_name: None, category: ResultCategory::Boolean },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Null, field_name: None, category: ResultCategory::Null },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::String, field_name: Some("txid"), category: ResultCategory::BitcoinTxid },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Hex, field_name: Some("transactionid"), category: ResultCategory::BitcoinTxid },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::String, field_name: Some("blockhash"), category: ResultCategory::BitcoinBlockHash },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::String, field_name: Some("script_pubkey"), category: ResultCategory::BitcoinScriptPubKey },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Hex, field_name: Some("script_pubkey"), category: ResultCategory::BitcoinScriptPubKey },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::String, field_name: Some("script"), category: ResultCategory::BitcoinScript },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Hex, field_name: Some("script"), category: ResultCategory::BitcoinScript },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::String, field_name: Some("redeemscript"), category: ResultCategory::BitcoinScript },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Hex, field_name: Some("redeemscript"), category: ResultCategory::BitcoinScript },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::String, field_name: Some("witnessscript"), category: ResultCategory::BitcoinScript },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Hex, field_name: Some("witnessscript"), category: ResultCategory::BitcoinScript },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::String, field_name: Some("hash_or_height"), category: ResultCategory::HashOrHeight },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("hash_or_height"), category: ResultCategory::HashOrHeight },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Wildcard, field_name: Some("hash_or_height"), category: ResultCategory::HashOrHeight },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("amount"), category: ResultCategory::BitcoinAmount },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("balance"), category: ResultCategory::BitcoinAmount },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("balance"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("fee_rate"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("estimated_feerate"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("maxfeerate"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("maxburnamount"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("relayfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("incrementalfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("incrementalrelayfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("mempoolminfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("minrelaytxfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("total_fee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: Some("blockmintxfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Amount, field_name: None, category: ResultCategory::BitcoinAmount },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("fee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("rate"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("feerate"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("maxfeerate"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("maxburnamount"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("relayfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("incrementalfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("incrementalrelayfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("mempoolminfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("minrelaytxfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("difficulty"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("probability"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("percentage"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("fee_rate"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("port"), category: ResultCategory::Port },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("nrequired"), category: ResultCategory::SmallInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("minconf"), category: ResultCategory::SmallInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("maxconf"), category: ResultCategory::SmallInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("locktime"), category: ResultCategory::SmallInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("version"), category: ResultCategory::SmallInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("verbosity"), category: ResultCategory::SmallInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("checklevel"), category: ResultCategory::SmallInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("n"), category: ResultCategory::SmallInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("blocks"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("maxtries"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("height"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("count"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("index"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("size"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("time"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("conf_target"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("skip"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("nodeid"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("peer_id"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("wait"), category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("changepos"), category: ResultCategory::SignedInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("confirmations"), category: ResultCategory::SignedInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("nblocks"), category: ResultCategory::SignedInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Hex, field_name: Some("txid"), category: ResultCategory::BitcoinTxid },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Hex, field_name: Some("blockhash"), category: ResultCategory::BitcoinBlockHash },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Hex, field_name: None, category: ResultCategory::String },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Array, field_name: Some("addresses"), category: ResultCategory::StringArray },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Array, field_name: Some("keys"), category: ResultCategory::StringArray },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Array, field_name: Some("stats"), category: ResultCategory::StringArray },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Array, field_name: Some("tx"), category: ResultCategory::TxidArray },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Array, field_name: Some("txids"), category: ResultCategory::TxidArray },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Array, field_name: Some("wallets"), category: ResultCategory::StringArray },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Array, field_name: None, category: ResultCategory::StringArray },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Object, field_name: Some("options"), category: ResultCategory::BitcoinObject },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Object, field_name: Some("query_options"), category: ResultCategory::BitcoinObject },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Object, field_name: None, category: ResultCategory::BitcoinObject },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("verificationprogress"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("difficulty"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("networkhashps"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("incrementalrelayfee"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("count_tok"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("size_tok"), category: ResultCategory::Float },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: None, category: ResultCategory::LargeInteger },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Timestamp, field_name: None, category: ResultCategory::Timestamp },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::NoneType, field_name: None, category: ResultCategory::None },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Any, field_name: None, category: ResultCategory::Any },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Elision, field_name: None, category: ResultCategory::Elision },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Range, field_name: None, category: ResultCategory::Range },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::String, field_name: Some("dummy"), category: ResultCategory::Dummy },
    ResultCategoryRule { rpc_type: ResultRpcJsonType::Number, field_name: Some("dummy"), category: ResultCategory::Dummy },
];

fn categorize_result_field(rpc_type: &str, field: &str) -> ResultCategory {
    let field_norm = normalize_field_name(field);
    let rpc_json_type = ResultRpcJsonType::from_str(rpc_type);

    let mut catchall: Option<ResultCategory> = None;

    for rule in RESULT_CATEGORY_RULES {
        if rule.rpc_type == rpc_json_type {
            match rule.field_name {
                Some(rule_field) if field_norm == normalize_field_name(rule_field) => {
                    return rule.category;
                }
                None => catchall = Some(rule.category),
                _ => {}
            }
        }
    }

    catchall.unwrap_or_else(|| {
        panic!(
            "No RESULT_CATEGORY_RULES match for rpc_type='{rpc_type}' field='{field}'. Add an explicit rule."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_field_name() {
        let normalized = normalize_field_name("A_b- C");
        assert_eq!(normalized, "abc");
    }

    #[test]
    fn test_map_parameter_type_to_rust() {
        let address = map_parameter_type_to_rust("string", "address");
        assert_eq!(address, "bitcoin::Address");

        let blockhash = map_parameter_type_to_rust("string", "blockhash");
        assert_eq!(blockhash, "bitcoin::BlockHash");

        let txid = map_parameter_type_to_rust("string", "txid");
        assert_eq!(txid, "bitcoin::Txid");

        let script_pubkey = map_parameter_type_to_rust("string", "script_pubkey");
        assert_eq!(script_pubkey, "bitcoin::ScriptBuf");

        let script = map_parameter_type_to_rust("string", "script");
        assert_eq!(script, "bitcoin::ScriptBuf");

        let redeemscript = map_parameter_type_to_rust("string", "redeemscript");
        assert_eq!(redeemscript, "bitcoin::ScriptBuf");

        let witnessscript = map_parameter_type_to_rust("string", "witnessscript");
        assert_eq!(witnessscript, "bitcoin::ScriptBuf");

        let generic_string = map_parameter_type_to_rust("string", "generic");
        assert_eq!(generic_string, "String");

        let number = map_parameter_type_to_rust("number", "any");
        assert_eq!(number, "i64");

        let bool_type = map_parameter_type_to_rust("bool", "any");
        assert_eq!(bool_type, "bool");

        let object = map_parameter_type_to_rust("object", "any");
        assert_eq!(object, "serde_json::Value");

        let array = map_parameter_type_to_rust("array", "any");
        assert_eq!(array, "Vec<serde_json::Value>");

        let amounts = map_parameter_type_to_rust("object", "amounts");
        assert!(
            amounts.contains("HashMap")
                && amounts.contains("Address")
                && amounts.contains("bitcoin::Amount")
        );

        let subtractfeefrom = map_parameter_type_to_rust("array", "subtractfeefrom");
        assert!(
            subtractfeefrom.contains("Vec")
                && subtractfeefrom.contains("Address")
                && subtractfeefrom.contains("NetworkUnchecked")
        );

        let recipients = map_parameter_type_to_rust("array", "recipients");
        assert!(recipients.contains("SendallRecipient"));

        let template_request = map_parameter_type_to_rust("object", "template_request");
        assert!(template_request.contains("GetBlockTemplateRequest"));

        let range = map_parameter_type_to_rust("range", "any");
        assert_eq!(range, "serde_json::Value");

        let unknown = map_parameter_type_to_rust("unknown", "any");
        assert_eq!(unknown, "serde_json::Value");
    }

    #[test]
    fn test_map_result_type_to_rust() {
        assert_eq!(map_result_type_to_rust("hex", "txid", ""), "bitcoin::Txid");
        assert_eq!(map_result_type_to_rust("hex", "data", ""), "String");
        assert_eq!(map_result_type_to_rust("number", "height", ""), "u64");
        assert_eq!(map_result_type_to_rust("number", "", "Current difficulty value"), "f64");
        assert_eq!(map_result_type_to_rust("boolean", "permitbaremultisig", ""), "bool");
        assert_eq!(map_result_type_to_rust("unknown_type", "field", ""), "serde_json::Value");
    }
}
