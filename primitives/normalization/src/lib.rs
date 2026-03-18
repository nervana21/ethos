// SPDX-License-Identifier: CC0-1.0

//! Shared normalization and canonicalization helpers used across Ethos crates.
//!
//! This crate is the single source of truth for:
//! - Loading normalization presets for Bitcoin Core RPCs.
//! - Mapping adapter-specific method names (e.g. \"listunspent\") to canonical
//!   PascalCase names (e.g. \"ListUnspent\").
//! - Suggesting canonical keys when a method is unmapped.
//!
//! Crates like `ethos-codegen` and `ethos-adapters` depend on this to keep
//! their response and element naming logic in sync.

use serde_json::{self, Value};

/// Embed normalization from workspace for Bitcoin Core.
const BITCOIN_NORMALIZATION_JSON: &str =
    include_str!("../../../resources/adapters/normalization/bitcoin.json");

/// Relative dirs (from workspace root) for the two copies of each normalization JSON file.
/// Used for error messages and by the pipeline when writing suggested mappings.
pub const NORMALIZATION_JSON_DIRS: [&str; 2] =
    ["compiler/codegen/resources/adapters/normalization", "resources/adapters/normalization"];

/// Optional context for unmapped RPC methods, used to build a rich error message
/// with category and description from the schema.
#[derive(Debug, Default, Clone)]
pub struct UnmappedMethodContext<'a> {
    /// Schema category (e.g. \"hidden\", \"wallet\", \"network\").
    pub category: Option<&'a str>,
    /// Method description or summary from the schema (first line used in error).
    pub description: Option<&'a str>,
}

/// A single suggested mapping to add to the normalization JSON files.
#[derive(Debug, Clone)]
pub struct SuggestedMapping {
    /// Adapter RPC method name (lowercase value in JSON).
    pub rpc_method: String,
    /// Suggested PascalCase key for method_mappings.
    pub suggested_key: String,
}

/// Error when one or more RPC methods have no mapping in the normalization presets.
#[derive(Debug)]
pub struct UnmappedMethodsError {
    /// Suggested entries to add to both normalization JSON files.
    pub suggestions: Vec<SuggestedMapping>,
}

/// Strict registry-driven conversion: adapter-specific RPC → canonical → snake_case
///
/// - protocol: \"bitcoin_core\"
/// - rpc_method: adapter-specific RPC method (e.g., \"getblockchaininfo\", \"getinfo\")
///
/// Errors if the preset is missing or the rpc_method has no mapping.
pub fn bitcoin_canonical_from_adapter_method(
    rpc_method: &str,
    context: Option<&UnmappedMethodContext<'_>>,
) -> Result<String, String> {
    canonical_from_adapter_method("bitcoin_core", rpc_method, context)
}

/// Resolves the canonical PascalCase name from an adapter-specific RPC using
/// normalization presets. If the method is unmapped and [context] is provided,
/// the error message includes suggested mapping, category, and description.
pub fn canonical_from_adapter_method(
    protocol: &str,
    rpc_method: &str,
    context: Option<&UnmappedMethodContext<'_>>,
) -> Result<String, String> {
    let (preset_json_str, impl_key) = match protocol {
        "bitcoin_core" => (BITCOIN_NORMALIZATION_JSON, "bitcoin_core"),
        other => return Err(format!("Unsupported protocol '{}'. Supported: bitcoin_core", other)),
    };

    let preset: Value = serde_json::from_str(preset_json_str)
        .map_err(|e| format!("Failed to parse normalization preset for {}: {}", protocol, e))?;

    let mappings = preset
        .get("method_mappings")
        .and_then(|mm| mm.get(impl_key))
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            format!(
                "Normalization preset for '{}' missing method_mappings. File must define method_mappings.{}",
                protocol, impl_key
            )
        })?;

    // Build reverse map: adapter-specific -> canonical (PascalCase)
    for (canonical, adapter_specific_val) in mappings.iter() {
        if let Some(adapter_specific) = adapter_specific_val.as_str() {
            if adapter_specific == rpc_method {
                return Ok(canonical.to_string());
            }
        }
    }

    let filename = if protocol == "bitcoin_core" { "bitcoin" } else { protocol };
    let suggested = suggest_canonical_key(rpc_method);
    let path_list: String = NORMALIZATION_JSON_DIRS
        .iter()
        .enumerate()
        .map(|(i, dir)| format!("  ({}) {}/{}.json", i + 1, dir, filename))
        .collect::<Vec<_>>()
        .join("\n");

    let mut msg = format!(
        "Unmapped RPC method '{}' for '{}'.\n\
         When using the compiler pipeline, suggested mappings are written automatically to both \
         normalization JSON files—review the changes (e.g. `git diff`) and re-run.\n\
         Otherwise add to method_mappings.{} in:\n\
         {}\n\
         Suggested entry:\n  \"{}\": \"{}\"",
        rpc_method, protocol, impl_key, path_list, suggested, rpc_method,
    );

    if let Some(ctx) = context {
        if let Some(cat) = ctx.category {
            if !cat.is_empty() {
                msg.push_str(&format!("\n  Category: {}", cat));
            }
        }
        if let Some(desc) = ctx.description {
            let first_line = desc.lines().next().unwrap_or("").trim();
            if !first_line.is_empty() {
                const MAX_DESC: usize = 120;
                let summary = if first_line.len() > MAX_DESC {
                    format!("{}...", &first_line[..MAX_DESC])
                } else {
                    first_line.to_string()
                };
                msg.push_str(&format!("\n  Summary: {}", summary));
            }
        }
    }

    Err(msg)
}

/// Suggests a PascalCase canonical key for an unmapped lowercase RPC method name.
/// Uses a greedy word-boundary split so e.g. \"getprivatebroadcastinfo\" → \"GetPrivateBroadcastInfo\".
pub fn suggest_canonical_key(rpc_method: &str) -> String {
    // Known acronym- or brand-like names that don't follow simple word capitalization.
    // Keep this list small and reserved for truly exceptional cases.
    match rpc_method {
        // Bitcoin Core OpenRPC helper method: keep "RPC" fully capitalized.
        "getopenrpcinfo" => return "GetOpenRpcInfo".to_string(),
        _ => {}
    }

    let lower = rpc_method.to_lowercase();
    let words = segment_lowercase_method_name(&lower);
    words.iter().map(|w| capitalize(w)).collect::<String>()
}

/// Validates that every method has a mapping for the given protocol.
/// Returns an error listing all unmapped methods with suggested keys so the caller
/// can write them directly into the normalization files.
pub fn validate_method_mappings(
    protocol: &str,
    methods: &[String],
) -> Result<(), UnmappedMethodsError> {
    let mut suggestions = Vec::new();
    for m in methods {
        if canonical_from_adapter_method(protocol, m, None).is_err() {
            suggestions.push(SuggestedMapping {
                rpc_method: m.clone(),
                suggested_key: suggest_canonical_key(m),
            });
        }
    }
    if suggestions.is_empty() {
        Ok(())
    } else {
        Err(UnmappedMethodsError { suggestions })
    }
}

/// Known method-name segments (verbs and nouns) for greedy longest-match segmentation.
/// Sorted by length descending so longer matches win (e.g. "broadcast" before "cast").
const METHOD_WORDS: &[&str] = &[
    "notifications",
    "transactions",
    "descendants",
    "descriptors",
    "prioritised",
    "transaction",
    "blockchain",
    "connection",
    "deployment",
    "descriptor",
    "difficulty",
    "disconnect",
    "invalidate",
    "passphrase",
    "prioritise",
    "reconsider",
    "validation",
    "addresses",
    "ancestors",
    "broadcast",
    "enumerate",
    "groupings",
    "interface",
    "scheduler",
    "activity",
    "balances",
    "echojson",
    "estimate",
    "finalize",
    "generate",
    "multisig",
    "precious",
    "received",
    "simulate",
    "spending",
    "template",
    "validate",
    "abandon",
    "address",
    "analyze",
    "balance",
    "cluster",
    "combine",
    "convert",
    "diagram",
    "display",
    "echoipc",
    "encrypt",
    "keypool",
    "logging",
    "mempool",
    "message",
    "migrate",
    "network",
    "package",
    "private",
    "process",
    "restore",
    "signers",
    "unspent",
    "wallets",
    "openrpc",
    "accept",
    "active",
    "backup",
    "banned",
    "change",
    "create",
    "decode",
    "derive",
    "export",
    "filter",
    "funded",
    "header",
    "height",
    "import",
    "labels",
    "memory",
    "mining",
    "orphan",
    "phrase",
    "pruned",
    "refill",
    "remove",
    "rescan",
    "schema",
    "script",
    "states",
    "submit",
    "totals",
    "unload",
    "update",
    "uptime",
    "verify",
    "wallet",
    "abort",
    "added",
    "block",
    "chain",
    "clear",
    "count",
    "entry",
    "funds",
    "index",
    "label",
    "proof",
    "prune",
    "psbts",
    "queue",
    "since",
    "smart",
    "state",
    "stats",
    "addr",
    "best",
    "bump",
    "dump",
    "echo",
    "flag",
    "from",
    "fund",
    "hash",
    "help",
    "info",
    "join",
    "json",
    "keys",
    "list",
    "load",
    "lock",
    "many",
    "mock",
    "node",
    "peer",
    "ping",
    "prev",
    "priv",
    "psbt",
    "rate",
    "save",
    "scan",
    "send",
    "sign",
    "stop",
    "sync",
    "test",
    "time",
    "tips",
    "utxo",
    "wait",
    "with",
    "add",
    "all",
    "ban",
    "dir",
    "fee",
    "for",
    "get",
    "ipc",
    "key",
    "man",
    "msg",
    "net",
    "new",
    "out",
    "raw",
    "rpc",
    "set",
    "tip",
    "zmq",
    "by",
    "hd",
    "ps",
    "to",
    "tx",
];

fn segment_lowercase_method_name(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut rest = s;

    while !rest.is_empty() {
        let mut matched = false;
        for word in METHOD_WORDS.iter() {
            if word.len() <= rest.len()
                && rest.as_bytes().get(..word.len()) == Some(word.as_bytes())
            {
                words.push((*word).to_string());
                rest = &rest[word.len()..];
                matched = true;
                break;
            }
        }
        if !matched {
            // No known word: take one character as a segment (capitalize it)
            let (ch, next) =
                rest.chars().next().map(|c| (c, &rest[c.len_utf8()..])).unwrap_or((' ', ""));
            rest = next;
            if ch.is_alphabetic() {
                words.push(ch.to_string());
            }
        }
    }
    words
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
