// SPDX-License-Identifier: CC0-1.0

use ir::RpcDef;
use normalization::UnmappedMethodContext;
pub use normalization::{SuggestedMapping, UnmappedMethodsError};
use types::Argument;

/// Validates that every method has a mapping for the given protocol.
/// Returns an error listing all unmapped methods with suggested keys so the caller
/// can write them directly into the normalization files.
pub fn validate_method_mappings(
    protocol: &str,
    methods: &[RpcDef],
) -> Result<(), UnmappedMethodsError> {
    let method_names: Vec<String> = methods.iter().map(|m| m.name.clone()).collect();
    normalization::validate_method_mappings(protocol, &method_names)
}

/// Strict registry-driven conversion: adapter-specific RPC → canonical → snake_case
///
/// - protocol: "bitcoin_core"
/// - rpc_method: adapter-specific RPC method (e.g., "getblockchaininfo", "getinfo")
///
/// Errors if the preset is missing or the rpc_method has no mapping.
pub fn protocol_rpc_method_to_rust_name(
    protocol: &str,
    rpc_method: &str,
) -> Result<String, String> {
    let canonical = canonical_from_adapter_method(protocol, rpc_method, None)?;
    Ok(pascal_to_snake_case(&canonical))
}

/// Like [protocol_rpc_method_to_rust_name] but when the method is unmapped the error
/// includes suggested mapping, category, and description from [UnmappedMethodContext].
pub fn protocol_rpc_method_to_rust_name_with_context(
    protocol: &str,
    rpc_method: &str,
    context: UnmappedMethodContext<'_>,
) -> Result<String, String> {
    let canonical = canonical_from_adapter_method(protocol, rpc_method, Some(&context))?;
    Ok(pascal_to_snake_case(&canonical))
}

/// Convert camelCase to snake_case
pub fn rpc_method_to_rust_name(rpc_method: &str) -> String {
    if rpc_method.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
        return pascal_to_snake_case(rpc_method);
    }

    let sanitized = sanitize_external_identifier(rpc_method);
    camel_to_snake_case(&sanitized)
}

/// Suggests a PascalCase canonical key for an unmapped lowercase RPC method name.
/// Uses a greedy word-boundary split so e.g. "getprivatebroadcastinfo" → "GetPrivateBroadcastInfo".
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
    "processed",
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

/// Resolves the canonical PascalCase name from an adapter-specific RPC using
/// normalization presets. If the method is unmapped and [context] is provided,
/// the error message includes suggested mapping, category, and description.
pub use normalization::canonical_from_adapter_method;

/// Convert camelCase to snake_case
fn camel_to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = input.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            // Check if previous character is lowercase or if this is the start of a new word
            if let Some(prev) = chars.get(i - 1) {
                if prev.is_lowercase()
                    || (i > 1 && chars.get(i - 2).is_some_and(|p| p.is_lowercase()))
                {
                    result.push('_');
                }
            }
        }
        result.push(c.to_lowercase().next().unwrap_or(*c));
    }

    result
}

/// Bitcoin compound terms that should be kept together when converting to snake_case.
/// These are domain-specific terms where the PascalCase form (e.g., "TxOut") should
/// become a single snake_case word (e.g., "txout") rather than being split (e.g., "tx_out").
const BITCOIN_COMPOUND_TERMS: &[&str] =
    &["AddrMan", "HashPs", "PrevOut", "PrivateBroadcast", "PubKey", "TxOut"];

/// Converts a PascalCase string to snake_case
///
/// This function is used to convert RPC method names from their PascalCase type name form
/// (e.g., `GetBlockchainInfo`, `AddNode`) to idiomatic Rust snake_case function names
/// (e.g., `get_blockchain_info`, `add_node`).
///
/// Bitcoin compound terms like "TxOut" are preserved as single words (e.g., "txout")
/// rather than being split (e.g., "tx_out").
///
/// # Examples
/// ```
/// use ethos_codegen::utils::pascal_to_snake_case;
/// assert_eq!(pascal_to_snake_case("GetBlockchainInfo"), "get_blockchain_info");
/// assert_eq!(pascal_to_snake_case("AddNode"), "add_node");
/// assert_eq!(pascal_to_snake_case("GetBalance"), "get_balance");
/// assert_eq!(pascal_to_snake_case("ScanTxOutSet"), "scan_txout_set");
/// assert_eq!(pascal_to_snake_case("GetTxOutProof"), "get_txout_proof");
/// ```
pub fn pascal_to_snake_case(input: &str) -> String {
    // First, replace compound terms with placeholders to preserve them
    // Use \x01 as start marker and \x02 as end marker
    let mut processed = input.to_string();
    for term in BITCOIN_COMPOUND_TERMS {
        let replacement = term.to_lowercase();
        processed = processed.replace(term, &format!("\x01{}\x02", replacement));
    }

    // Now do standard PascalCase to snake_case conversion
    let mut result = String::new();
    let mut in_placeholder = false;

    for c in processed.chars() {
        if c == '\x01' {
            // Starting a compound term - add underscore if needed
            if !result.is_empty() && !result.ends_with('_') {
                result.push('_');
            }
            in_placeholder = true;
            continue;
        }
        if c == '\x02' {
            in_placeholder = false;
            continue;
        }

        if in_placeholder {
            // Inside a compound term placeholder - just add the character as-is
            result.push(c);
        } else if c.is_uppercase() && !result.is_empty() {
            result.push('_');
            result.push(c.to_lowercase().next().unwrap_or(c));
        } else {
            result.push(c.to_lowercase().next().unwrap_or(c));
        }
    }

    result
}

/// Capitalize the first letter of a string
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Convert snake_case to PascalCase
pub fn snake_to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// RPC/IR concatenated or camelCase names -> idiomatic snake_case Rust identifiers.
/// Keep alphabetically sorted by the RPC/wire key.
const CONCAT_TO_SNAKE: &[(&str, &str)] = &[
    ("addednode", "added_node"),
    ("addrbind", "addr_bind"),
    ("addrlocal", "addr_local"),
    ("ancestorcount", "ancestor_count"),
    ("ancestorfees", "ancestor_fees"),
    ("ancestorsize", "ancestor_size"),
    ("avgfeerate", "avg_fee_rate"),
    ("avgtxsize", "avg_tx_size"),
    ("bantime", "ban_time"),
    ("bestblockhash", "best_block_hash"),
    ("bestblock", "best_block"),
    ("bip32derivs", "bip32_derivs"),
    ("birthtime", "birth_time"),
    ("blockhash", "block_hash"),
    ("blockheight", "block_height"),
    ("blockindex", "block_index"),
    ("blockmintxfee", "block_min_tx_fee"),
    ("blocktime", "block_time"),
    ("bogosize", "bogo_size"),
    ("bytesrecv", "bytes_recv"),
    ("bytesrecv_per_msg", "bytes_recv_per_msg"),
    ("bytesrecvpermsg", "bytes_recv_per_msg"),
    ("bytessent", "bytes_sent"),
    ("bytessent_per_msg", "bytes_sent_per_msg"),
    ("bytessentpermsg", "bytes_sent_per_msg"),
    ("bucket/position", "bucket_position"),
    ("chainwork", "chain_work"),
    ("chunkweight", "chunk_weight"),
    ("coinbaseaux", "coinbase_aux"),
    ("coinbasevalue", "coinbase_value"),
    ("conntime", "conn_time"),
    ("curtime", "cur_time"),
    ("currentblocktx", "current_block_tx"),
    ("currentblockweight", "current_block_weight"),
    ("descendantcount", "descendant_count"),
    ("descendantsize", "descendant_size"),
    ("endrange", "end_range"),
    ("feerate", "fee_rate"),
    ("filepath", "file_path"),
    ("final_scriptSig", "final_script_sig"),
    ("final_scriptwitness", "final_script_witness"),
    ("fullrbf", "full_rbf"),
    ("hdkeypath", "hd_key_path"),
    ("hdmasterfingerprint", "hd_master_fingerprint"),
    ("hdseedid", "hd_seed_id"),
    ("hexstring", "hex_string"),
    ("incrementalfee", "incremental_fee"),
    ("incrementalrelayfee", "incremental_relay_fee"),
    ("inflight", "in_flight"),
    ("inmempool", "in_mempool"),
    ("initialblockdownload", "initial_block_download"),
    ("iscompressed", "is_compressed"),
    ("ismine", "is_mine"),
    ("isrange", "is_range"),
    ("isscript", "is_script"),
    ("issolvable", "is_solvable"),
    ("iswatchonly", "is_watch_only"),
    ("iswitness", "is_witness"),
    ("hasprivatekeys", "has_private_keys"),
    ("keypoolsize", "key_pool_size"),
    ("lastblock", "last_block"),
    ("lastprocessedblock", "last_processed_block"),
    ("lastrecv", "last_recv"),
    ("lastsend", "last_send"),
    ("leftmempool", "left_mempool"),
    ("limitclustercount", "limit_cluster_count"),
    ("limitclustersize", "limit_cluster_size"),
    ("localaddresses", "local_addresses"),
    ("localrelay", "local_relay"),
    ("localservices", "local_services"),
    ("localservicesnames", "local_services_names"),
    ("locktime", "lock_time"),
    ("localrelay", "local_relay"),
    ("localservices", "local_services"),
    ("localservicesnames", "local_services_names"),
    ("longpollid", "longpoll_id"),
    ("maxburnamount", "max_burn_amount"),
    ("maxconf", "max_conf"),
    ("maxdatacarriersize", "max_data_carrier_size"),
    ("maxfee", "max_fee"),
    ("maxfeerate", "max_fee_rate"),
    ("maxtxsize", "max_tx_size"),
    ("maxmempool", "max_mempool"),
    ("mempoolconflicts", "mempool_conflicts"),
    ("mempoolminfee", "mempool_min_fee"),
    ("merkleroot", "merkle_root"),
    ("mediantime", "median_time"),
    ("mediantxsize", "median_tx_size"),
    ("minconf", "min_conf"),
    ("minfeefilter", "min_fee_filter"),
    ("minfee", "min_fee"),
    ("minfeerate", "min_fee_rate"),
    ("minping", "min_ping"),
    ("minrelaytxfee", "min_relay_tx_fee"),
    ("mintxsize", "min_tx_size"),
    ("nchaintx", "n_chain_tx"),
    ("noncerange", "nonce_range"),
    ("networkactive", "network_active"),
    ("networkhashps", "network_hashps"),
    ("newpassphrase", "new_passphrase"),
    ("nextblockhash", "next_block_hash"),
    ("oldpassphrase", "old_passphrase"),
    ("origfee", "orig_fee"),
    ("permitsigdata", "permit_sig_data"),
    ("permitbaremultisig", "permit_bare_multisig"),
    ("pingtime", "ping_time"),
    ("pingwait", "ping_wait"),
    ("pooledtx", "pooled_tx"),
    ("previousblockhash", "previous_block_hash"),
    ("prevtxs", "prev_txs"),
    ("privkeys", "priv_keys"),
    ("pruneheight", "prune_height"),
    ("protocolversion", "protocol_version"),
    ("pubkeys", "pub_keys"),
    ("pubnonce", "pub_nonce"),
    ("redeemScript", "redeem_script"),
    ("redeemscript", "redeem_script"),
    ("relayfee", "relay_fee"),
    ("relaytxes", "relay_txes"), // Preserve Core's pluralization "txes"
    ("rulename", "rule_name"),
    ("scriptPubKey", "script_pubkey"),
    ("scriptSig", "script_sig"),
    ("servicesnames", "services_names"),
    ("sigops", "sig_ops"),
    ("sigoplimit", "sigop_limit"),
    ("sighash", "sig_hash"),
    ("sighashtype", "sighash_type"),
    ("sigsrequired", "sigs_required"),
    ("sizelimit", "size_limit"),
    ("spentby", "spent_by"),
    ("startrange", "start_range"),
    ("startingheight", "starting_height"),
    ("strippedsize", "stripped_size"),
    ("subtractfeefromamount", "subtract_fee_from_amount"),
    ("subtype", "sub_type"),
    ("swtotal_size", "sw_total_size"),
    ("swtotal_weight", "sw_total_weight"),
    ("swtxs", "sw_txs"),
    ("timeoffset", "time_offset"),
    ("timereceived", "time_received"),
    ("timemillis", "time_millis"),
    ("totalbytesrecv", "total_bytes_recv"),
    ("totalbytessent", "total_bytes_sent"),
    ("totalfee", "total_fee"),
    ("totalconfirmed", "total_confirmed"),
    ("transactionid", "transaction_id"),
    ("txcount", "tx_count"),
    ("txinwitness", "tx_in_witness"),
    ("txrate", "tx_rate"),
    ("unbroadcastcount", "unbroadcast_count"),
    ("uploadtarget", "upload_target"),
    ("vbavailable", "vb_available"),
    ("vbrequired", "vb_required"),
    ("verificationprogress", "verification_progress"),
    ("versionHex", "version_hex"),
    ("vsize", "v_size"),
    ("walletconflicts", "wallet_conflicts"),
    ("walletname", "wallet_name"),
    ("walletversion", "wallet_version"),
    ("weightlimit", "weight_limit"),
    ("withintarget", "within_target"),
    ("witnessScript", "witness_script"),
    ("witnessscript", "witness_script"),
    ("wtxid", "w_txid"),
];

/// Sanitizes external identifiers (e.g. RPC schemas) to be valid Rust identifiers.
/// Prefixes with `field_` when the result would start with a digit (e.g. `10th_percentile_feerate` -> `field_10th_percentile_feerate`).
pub fn sanitize_external_identifier(name: &str) -> String {
    // Handle reserved keywords
    match name {
        "type" => return "r#type".to_string(),
        "self" => return "self_".to_string(),
        "super" => return "super_".to_string(),
        "crate" => return "crate_".to_string(),
        _ => {}
    }
    // Concatenated RPC names -> idiomatic snake_case
    if let Some((_, rust_name)) = CONCAT_TO_SNAKE.iter().find(|(k, _)| *k == name) {
        return (*rust_name).to_string();
    }
    // Replace hyphens with underscores and remove other invalid characters
    let sanitized: String =
        name.replace('-', "_").chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
    // Rust identifiers cannot start with a digit; use field_ prefix so #[serde(rename)] keeps JSON key correct
    if sanitized.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("field_{sanitized}")
    } else {
        sanitized
    }
}

/// Sanitizes a type name for use as a Rust type or struct name (e.g. in generated code).
/// Replaces `/` and `-` with `_`, then converts to upper camel case (PascalCase) so
/// `GetrawaddrmanBucket/position` -> `GetrawaddrmanBucketPosition` and we avoid non_camel_case_types warnings.
pub fn sanitize_type_name_for_rust(name: &str) -> String {
    let mut with_underscores = name.replace('/', "_").replace('-', "_");
    with_underscores = with_underscores
        .replace("Lastprocessedblock", "Last_Processed_Block")
        .replace("Pubnonces", "Pub_Nonces");
    // Convert to PascalCase: split on _, capitalize each segment, join
    with_underscores
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(c).collect(),
            }
        })
        .collect()
}

/// Check if a method needs parameter reordering
pub fn needs_parameter_reordering(args: &[Argument]) -> bool {
    // Simple heuristic: if we have more than 3 parameters, use a struct
    args.len() > 3
}

/// Reorder arguments for better Rust API
pub fn reorder_arguments_for_rust_signature(args: &[Argument]) -> (Vec<Argument>, Vec<usize>) {
    // For now, just return the original order
    // In the future, this could implement more sophisticated reordering
    let mapping: Vec<usize> = (0..args.len()).collect();
    (args.to_vec(), mapping)
}

#[cfg(test)]
mod tests {
    use ir::RpcDef;

    use super::*;

    fn minimal_argument() -> Argument {
        Argument {
            names: vec![],
            description: String::new(),
            oneline_description: String::new(),
            also_positional: false,
            type_str: None,
            required: false,
            hidden: false,
            type_: String::new(),
        }
    }

    fn rpc_def_with_name(name: &str) -> RpcDef {
        RpcDef {
            name: name.to_string(),
            description: String::new(),
            params: vec![],
            result: None,
            category: String::new(),
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_method_mappings() {
        let methods = vec![rpc_def_with_name("getblockchaininfo")];

        let out = validate_method_mappings("bitcoin_core", &methods);

        assert!(out.is_ok());

        let methods =
            vec![rpc_def_with_name("getblockchaininfo"), rpc_def_with_name("getnonexistentxyz")];

        let out = validate_method_mappings("bitcoin_core", &methods);

        let err = out.expect_err("unmapped method should yield error");
        assert_eq!(err.suggestions.len(), 1);
        assert_eq!(err.suggestions[0].rpc_method, "getnonexistentxyz");
    }

    #[test]
    fn test_protocol_rpc_method_to_rust_name() {
        let out = protocol_rpc_method_to_rust_name("bitcoin_core", "getblockchaininfo");

        assert_eq!(out.expect("mapped method"), "get_blockchain_info");

        let out = protocol_rpc_method_to_rust_name("other", "getinfo");

        assert!(out.is_err());
        assert!(out.expect_err("unsupported protocol").contains("Unsupported protocol"));

        let out = protocol_rpc_method_to_rust_name("bitcoin_core", "getnonexistentxyz");

        assert!(out.is_err());
        assert!(out.expect_err("unmapped method").contains("Unmapped RPC method"));
    }

    #[test]
    fn test_protocol_rpc_method_to_rust_name_with_context() {
        let out = protocol_rpc_method_to_rust_name_with_context(
            "bitcoin_core",
            "getblockchaininfo",
            UnmappedMethodContext::default(),
        );

        assert_eq!(out.expect("mapped method"), "get_blockchain_info");

        let ctx =
            UnmappedMethodContext { category: Some("hidden"), description: Some("Short summary.") };
        let out =
            protocol_rpc_method_to_rust_name_with_context("bitcoin_core", "getnonexistentxyz", ctx);

        let err = out.expect_err("unmapped method");
        assert!(err.contains("Unmapped RPC method"));
        assert!(err.contains("Category: hidden"));
        assert!(err.contains("Summary: Short summary."));
    }

    #[test]
    fn test_rpc_method_to_rust_name() {
        let out = rpc_method_to_rust_name("GetBlockchainInfo");

        assert_eq!(out, "get_blockchain_info");

        let out = rpc_method_to_rust_name("getBlockchainInfo");

        assert_eq!(out, "get_blockchain_info");

        let out = rpc_method_to_rust_name("");

        assert_eq!(out, "");
    }

    #[test]
    fn test_suggest_canonical_key() {
        let out = suggest_canonical_key("getblockchaininfo");

        assert_eq!(out, "GetBlockchainInfo");
    }

    #[test]
    fn test_bitcoin_core_method_words_exhaustive() {
        let json_str = include_str!("../../../resources/adapters/normalization/bitcoin.json");
        let preset: serde_json::Value =
            serde_json::from_str(json_str).expect("bitcoin.json must be valid JSON");
        let mappings = preset
            .get("method_mappings")
            .and_then(|mm| mm.get("bitcoin_core"))
            .and_then(|v| v.as_object())
            .expect("method_mappings.bitcoin_core must exist");
        let exceptions: std::collections::HashSet<&str> =
            ["getorphantxs", "scanblocks"].into_iter().collect();
        let mut mismatches = Vec::new();
        for (canonical, val) in mappings.iter() {
            let adapter_lower = match val.as_str() {
                Some(s) => s,
                None => continue,
            };
            if exceptions.contains(adapter_lower) {
                continue;
            }
            let suggested = suggest_canonical_key(adapter_lower);
            if suggested != *canonical {
                mismatches.push((adapter_lower.to_string(), canonical.to_string(), suggested));
            }
        }
        assert!(
            mismatches.is_empty(),
            "METHOD_WORDS is missing segments for these Bitcoin Core methods (adapter_name => expected canonical, got suggested):\n{:?}\n\
             Add the missing segment(s) to METHOD_WORDS in utils.rs.",
            mismatches
        );
    }

    #[test]
    fn test_canonical_from_adapter_method() {
        let out = canonical_from_adapter_method("other", "getinfo", None);

        assert!(out.is_err());
        assert!(out.expect_err("unsupported protocol").contains("Unsupported protocol"));

        let out = canonical_from_adapter_method("bitcoin_core", "getblockchaininfo", None);

        assert_eq!(out.expect("mapped"), "GetBlockchainInfo");

        let out = canonical_from_adapter_method("bitcoin_core", "getnonexistentxyz", None);

        let err = out.expect_err("unmapped");
        assert!(err.contains("Unmapped RPC method"));
        assert!(!err.contains("Category:"));
        assert!(!err.contains("Summary:"));

        let ctx = UnmappedMethodContext { category: Some(""), description: None };
        let out = canonical_from_adapter_method("bitcoin_core", "getnonexistentxyz", Some(&ctx));

        let err = out.expect_err("unmapped with empty category");
        assert!(!err.contains("Category:"));

        let ctx = UnmappedMethodContext { category: Some("hidden"), description: None };
        let out = canonical_from_adapter_method("bitcoin_core", "getnonexistentxyz", Some(&ctx));

        let err = out.expect_err("unmapped with category");
        assert!(err.contains("Category: hidden"));

        let ctx = UnmappedMethodContext { category: None, description: Some("") };
        let out = canonical_from_adapter_method("bitcoin_core", "getnonexistentxyz", Some(&ctx));

        let err = out.expect_err("unmapped with empty description");
        assert!(!err.contains("Summary:"));

        let ctx = UnmappedMethodContext { category: None, description: Some("Short line.") };
        let out = canonical_from_adapter_method("bitcoin_core", "getnonexistentxyz", Some(&ctx));

        let err = out.expect_err("unmapped with short description");
        assert!(err.contains("Summary: Short line."));

        let long_desc = "x".repeat(121);
        let ctx = UnmappedMethodContext { category: None, description: Some(&long_desc) };
        let out = canonical_from_adapter_method("bitcoin_core", "getnonexistentxyz", Some(&ctx));

        let err = out.expect_err("unmapped with long description");
        assert!(err.contains("Summary:"));
        assert!(err.contains("..."));
    }

    #[test]
    fn test_pascal_to_snake_case() {
        let out = pascal_to_snake_case("TxOut");

        assert_eq!(out, "txout");

        let out = pascal_to_snake_case("GetTxOut");

        assert_eq!(out, "get_txout");

        let out = pascal_to_snake_case("GetTxSpendingPrevOut");

        assert_eq!(out, "get_tx_spending_prevout");

        let out = pascal_to_snake_case("ScriptPubKey");

        assert_eq!(out, "script_pubkey");

        let out = pascal_to_snake_case("GetNetworkHashPs");

        assert_eq!(out, "get_network_hashps");

        let out = pascal_to_snake_case("GetAddrManInfo");

        assert_eq!(out, "get_addrman_info");

        let out = pascal_to_snake_case("GetPrivateBroadcastInfo");

        assert_eq!(out, "get_privatebroadcast_info");

        let out = pascal_to_snake_case("GetBlockchainInfo");

        assert_eq!(out, "get_blockchain_info");
    }

    #[test]
    fn test_capitalize() {
        let out = capitalize("");

        assert_eq!(out, "");

        let out = capitalize("hello");

        assert_eq!(out, "Hello");
    }

    #[test]
    fn test_snake_to_pascal_case() {
        let out = snake_to_pascal_case("");

        assert_eq!(out, "");

        let out = snake_to_pascal_case("a");

        assert_eq!(out, "A");

        let out = snake_to_pascal_case("get_blockchain_info");

        assert_eq!(out, "GetBlockchainInfo");
    }

    #[test]
    fn test_sanitize_external_identifier() {
        let out = sanitize_external_identifier("type");

        assert_eq!(out, "r#type");

        let out = sanitize_external_identifier("self");

        assert_eq!(out, "self_");

        let out = sanitize_external_identifier("super");

        assert_eq!(out, "super_");

        let out = sanitize_external_identifier("crate");

        assert_eq!(out, "crate_");

        let out = sanitize_external_identifier("foo");

        assert_eq!(out, "foo");

        let out = sanitize_external_identifier("foo-bar");

        assert_eq!(out, "foo_bar");

        let out = sanitize_external_identifier("foo@bar");

        assert_eq!(out, "foobar");

        // Rust identifiers cannot start with a digit; prefix with field_ and caller uses #[serde(rename)] for JSON key
        let out = sanitize_external_identifier("10th_percentile_feerate");
        assert_eq!(out, "field_10th_percentile_feerate");
    }

    #[test]
    fn test_needs_parameter_reordering() {
        let args: Vec<Argument> = (0..4).map(|_| minimal_argument()).collect();

        let out = needs_parameter_reordering(&args);

        assert!(out);

        let args: Vec<Argument> = (0..3).map(|_| minimal_argument()).collect();

        let out = needs_parameter_reordering(&args);

        assert!(!out);
    }

    #[test]
    fn test_reorder_arguments_for_rust_signature() {
        let args: Vec<Argument> = (0..2).map(|_| minimal_argument()).collect();

        let (reordered, mapping) = reorder_arguments_for_rust_signature(&args);

        assert_eq!(reordered.len(), 2);
        assert_eq!(mapping, vec![0, 1]);
    }

    #[test]
    fn test_sanitize_external_identifier_concat_to_snake() {
        for (rpc_name, expected) in super::CONCAT_TO_SNAKE {
            assert_eq!(sanitize_external_identifier(rpc_name), *expected);
        }
    }

    #[test]
    fn test_sanitize_type_name_for_rust() {
        assert_eq!(
            sanitize_type_name_for_rust("GetrawaddrmanBucket/position"),
            "GetrawaddrmanBucketPosition"
        );
        assert_eq!(
            sanitize_type_name_for_rust("SubmitpackageTx-results"),
            "SubmitpackageTxResults"
        );
        assert_eq!(sanitize_type_name_for_rust("DecodepsbtTx"), "DecodepsbtTx");
        assert_eq!(
            sanitize_type_name_for_rust("GetBalancesLastprocessedblock"),
            "GetBalancesLastProcessedBlock"
        );
        assert_eq!(
            sanitize_type_name_for_rust("DecodePsbtMusig2Pubnonces"),
            "DecodePsbtMusig2PubNonces"
        );
    }
}
