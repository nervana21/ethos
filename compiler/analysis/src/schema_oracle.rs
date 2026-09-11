//! Schema-oracle RPC fuzz: mutate within Δ, classify wire vs IR.
//!
//! Living Δ for Core RPC — dump/IR is the oracle. A call outcome is classified as:
//! - [`OracleClass::Ok`] — success and response matches `RpcDef.result`
//! - [`OracleClass::ExpectedReject`] — JSON-RPC error (state / validation)
//! - [`OracleClass::SchemaMismatch`] — success but response fails IR check (**oracle hit**)
//! - [`OracleClass::DecodeFail`] — optional Raw-type serde failure after IR ok
//! - [`OracleClass::TransportError`] — client/transport failure (not an RPC error body)
//!
//! Does not compete with Core in-process crash fuzz or Fuzzamoto snapshot full-system fuzz.
//! First vertical slice: positional param generation + IR response validation + pluggable invoker.

use std::collections::BTreeMap;

use ir::json_golden::validate_json_matches_type;
use ir::{ProtocolIR, RpcDef, TypeDef, TypeKind, VariantDef};
use serde_json::Value;
use thiserror::Error;

/// Classification of one oracle observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleClass {
    /// Success and response matches IR result type.
    Ok,
    /// Node returned a JSON-RPC error (often state / arg validation).
    ExpectedReject {
        /// JSON-RPC error code when known.
        code: Option<i32>,
        /// Error message / detail.
        message: String,
    },
    /// Success response does not match IR — primary schema-oracle finding.
    SchemaMismatch {
        /// Path-aware detail from [`validate_json_matches_type`].
        detail: String,
    },
    /// IR matched but an optional Raw serde decode failed.
    DecodeFail {
        /// Decode error detail.
        detail: String,
    },
    /// Transport / client failure before an RPC result or error body.
    TransportError {
        /// Transport error detail.
        detail: String,
    },
}

impl OracleClass {
    /// True when this class is a Δ / codegen finding (not a normal reject).
    pub fn is_oracle_finding(&self) -> bool {
        matches!(self, Self::SchemaMismatch { .. } | Self::DecodeFail { .. })
    }
}

/// One classified observation.
#[derive(Debug, Clone)]
pub struct OracleReport {
    /// RPC method name.
    pub method: String,
    /// Positional JSON-RPC params used.
    pub params: Vec<Value>,
    /// Classification.
    pub class: OracleClass,
}

/// Error from an invoker before/without a JSON-RPC result object.
#[derive(Debug, Error, Clone)]
pub enum InvokeError {
    /// JSON-RPC error object from the node.
    #[error("rpc error code={code:?}: {message}")]
    Rpc {
        /// JSON-RPC `error.code`.
        code: Option<i32>,
        /// JSON-RPC `error.message` or fallback text.
        message: String,
    },
    /// HTTP / client / spawn failure.
    #[error("transport: {0}")]
    Transport(String),
}

/// Parse a JSON-RPC `error` object body (as returned by ethos-bitcoind `TransportError::Rpc`).
///
/// Accepts either a JSON object string (`{"code":-8,"message":"..."}`) or opaque text.
pub fn invoke_error_from_rpc_body(body: &str) -> InvokeError {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        let code = v.get("code").and_then(|c| c.as_i64()).map(|c| c as i32);
        let message = v.get("message").and_then(|m| m.as_str()).unwrap_or(body).to_string();
        return InvokeError::Rpc { code, message };
    }
    InvokeError::Rpc { code: None, message: body.to_string() }
}

/// Pluggable RPC caller (mock in tests; HTTP via smoke binary).
pub trait RpcInvoker {
    /// Invoke `method` with positional `params`. `Ok` = JSON-RPC `result` value.
    fn invoke(&mut self, method: &str, params: &[Value]) -> Result<Value, InvokeError>;
}

/// Sync invoker backed by a mutable closure (wraps async transports with `block_on`).
pub struct ClosureInvoker<F>
where
    F: FnMut(&str, &[Value]) -> Result<Value, InvokeError>,
{
    /// Inner call.
    pub call: F,
}

impl<F> RpcInvoker for ClosureInvoker<F>
where
    F: FnMut(&str, &[Value]) -> Result<Value, InvokeError>,
{
    fn invoke(&mut self, method: &str, params: &[Value]) -> Result<Value, InvokeError> {
        (self.call)(method, params)
    }
}

/// Byte cursor for deterministic, fuzz-friendly value generation.
#[derive(Debug, Clone)]
pub struct ByteCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteCursor<'a> {
    /// Create a cursor over `data`.
    pub fn new(data: &'a [u8]) -> Self { Self { data, pos: 0 } }

    fn next_u8(&mut self) -> u8 {
        if self.pos >= self.data.len() {
            // Stable pad so empty inputs still produce deterministic shapes.
            return (self.pos as u8).wrapping_mul(31).wrapping_add(17);
        }
        let b = self.data[self.pos];
        self.pos += 1;
        b
    }

    fn next_bool(&mut self) -> bool { self.next_u8() & 1 == 1 }

    fn next_usize(&mut self, max_exclusive: usize) -> usize {
        if max_exclusive == 0 {
            return 0;
        }
        (self.next_u8() as usize) % max_exclusive
    }

    fn next_i64(&mut self) -> i64 {
        let mut buf = [0u8; 8];
        for slot in &mut buf {
            *slot = self.next_u8();
        }
        i64::from_le_bytes(buf)
    }

    fn next_string(&mut self, max_len: usize) -> String {
        let len = self.next_usize(max_len.saturating_add(1));
        let mut s = String::with_capacity(len);
        for _ in 0..len {
            // Printable ASCII for RPC-friendly strings.
            let c = b'a' + (self.next_u8() % 26);
            s.push(c as char);
        }
        s
    }
}

/// Generate positional JSON-RPC params for `rpc` from fuzz bytes.
///
/// Required params always emitted. First omitted optional stops the list (trailing omit).
pub fn generate_params(rpc: &RpcDef, data: &[u8]) -> Vec<Value> {
    let mut cur = ByteCursor::new(data);
    let mut out = Vec::with_capacity(rpc.params.len());
    for param in &rpc.params {
        if !param.required && !cur.next_bool() {
            break;
        }
        out.push(generate_value(&param.name, &param.param_type, &mut cur, 0));
    }
    out
}

/// Mutate an existing positional param list while staying loosely inside schema.
///
/// Empty `base` falls back to [`generate_params`].
pub fn mutate_params(rpc: &RpcDef, base: &[Value], data: &[u8]) -> Vec<Value> {
    if base.is_empty() {
        return generate_params(rpc, data);
    }
    let mut cur = ByteCursor::new(data);
    let mut out = base.to_vec();
    if out.is_empty() {
        return generate_params(rpc, data);
    }
    let idx = cur.next_usize(out.len());
    if let Some(param) = rpc.params.get(idx) {
        out[idx] = generate_value(&param.name, &param.param_type, &mut cur, 0);
    } else {
        out[idx] = Value::String(cur.next_string(8));
    }
    if cur.next_bool() && out.len() < rpc.params.len().saturating_add(2) {
        out.push(Value::String(cur.next_string(4)));
    }
    out
}

const MAX_GEN_DEPTH: usize = 4;

fn name_hint(s: &str) -> String { s.to_ascii_lowercase() }

fn looks_like_blockhash(hint: &str) -> bool {
    hint.contains("blockhash") || hint.contains("block_hash") || hint == "hash"
}

fn looks_like_txid(hint: &str) -> bool {
    hint.contains("txid") || hint.contains("transactionid") || hint.contains("wtxid")
}

fn looks_like_hex_hash(hint: &str, ty: &TypeDef) -> bool {
    if ty.protocol_type.as_deref() == Some("hex") {
        return true;
    }
    looks_like_blockhash(hint) || looks_like_txid(hint) || hint.contains("hash")
}

fn looks_like_verbosity(hint: &str) -> bool { hint.contains("verbosity") || hint == "verbose" }

fn generate_value(param_name: &str, ty: &TypeDef, cur: &mut ByteCursor<'_>, depth: usize) -> Value {
    if depth >= MAX_GEN_DEPTH {
        return Value::Null;
    }
    let hint = name_hint(param_name);
    match ty.kind {
        TypeKind::Optional => {
            if cur.next_bool() {
                return Value::Null;
            }
            let inner = ty.fields.as_ref().and_then(|f| f.first()).map(|fd| &fd.field_type);
            match inner {
                Some(inner_ty) => generate_value(param_name, inner_ty, cur, depth + 1),
                None => Value::Null,
            }
        }
        TypeKind::Union => {
            let variants = ty.union_variants.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
            if variants.is_empty() {
                return Value::Null;
            }
            let i = cur.next_usize(variants.len());
            generate_value(param_name, &variants[i].type_def, cur, depth + 1)
        }
        TypeKind::Array => {
            let n = cur.next_usize(3);
            let mut arr = Vec::with_capacity(n);
            if let Some(elem) =
                ty.homogeneous_array_element_type().or_else(|| ty.array_element_type())
            {
                for _ in 0..n {
                    arr.push(generate_value(param_name, elem, cur, depth + 1));
                }
            }
            Value::Array(arr)
        }
        TypeKind::Map => {
            let n = cur.next_usize(2);
            let mut map = serde_json::Map::new();
            let val_ty = ty.map_value_type();
            for _ in 0..n {
                let key = cur.next_string(8);
                let v = match val_ty {
                    Some(t) => generate_value(param_name, t, cur, depth + 1),
                    None => Value::Null,
                };
                map.insert(key, v);
            }
            Value::Object(map)
        }
        TypeKind::Object => {
            if ty.protocol_type.as_deref() == Some("array") {
                let n = cur.next_usize(2);
                let mut arr = Vec::with_capacity(n);
                if let Some(fields) = ty.fields.as_ref() {
                    if let Some(elem) = fields.first() {
                        for _ in 0..n {
                            arr.push(generate_value(
                                elem.key.as_ident().as_str(),
                                &elem.field_type,
                                cur,
                                depth + 1,
                            ));
                        }
                    }
                }
                return Value::Array(arr);
            }
            let mut map = serde_json::Map::new();
            if let Some(fields) = ty.fields.as_ref() {
                for field in fields {
                    if field.emit_in_struct == Some(false) {
                        continue;
                    }
                    if field.field_type.protocol_type.as_deref() == Some("elision") {
                        continue;
                    }
                    let Some(key) = field.key.json_key() else {
                        continue;
                    };
                    let required = field.required && field.force_optional != Some(true);
                    if !required && !cur.next_bool() {
                        continue;
                    }
                    map.insert(
                        key.to_string(),
                        generate_value(key, &field.field_type, cur, depth + 1),
                    );
                }
            }
            Value::Object(map)
        }
        TypeKind::Enum => generate_enum_value(ty, cur),
        TypeKind::Primitive | TypeKind::Alias | TypeKind::Custom =>
            generate_primitive(&hint, ty, cur),
    }
}

fn generate_enum_value(ty: &TypeDef, cur: &mut ByteCursor<'_>) -> Value {
    if let Some(variants) = ty.variants.as_ref() {
        if !variants.is_empty() {
            let i = cur.next_usize(variants.len());
            return variant_to_value(&variants[i]);
        }
    }
    generate_primitive(&name_hint(&ty.name), ty, cur)
}

fn variant_to_value(v: &VariantDef) -> Value {
    let wire = v.value.as_deref().unwrap_or(v.name.as_str());
    if let Ok(n) = wire.parse::<i64>() {
        return Value::Number(n.into());
    }
    Value::String(wire.to_string())
}

fn generate_hex(cur: &mut ByteCursor<'_>, nbytes: usize) -> Value {
    let mut s = String::with_capacity(nbytes * 2);
    for _ in 0..nbytes {
        let b = cur.next_u8();
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap_or('0'));
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap_or('0'));
    }
    Value::String(s)
}

fn generate_primitive(hint: &str, ty: &TypeDef, cur: &mut ByteCursor<'_>) -> Value {
    let p = ty.protocol_type.as_deref().unwrap_or(ty.name.as_str());

    if hint.contains("estimate_mode") {
        const MODES: &[&str] = &["UNSET", "ECONOMICAL", "CONSERVATIVE"];
        return Value::String(MODES[cur.next_usize(MODES.len())].to_string());
    }
    if hint.contains("sighashtype") {
        const SIGS: &[&str] = &[
            "DEFAULT",
            "ALL",
            "NONE",
            "SINGLE",
            "ALL|ANYONECANPAY",
            "NONE|ANYONECANPAY",
            "SINGLE|ANYONECANPAY",
        ];
        return Value::String(SIGS[cur.next_usize(SIGS.len())].to_string());
    }

    match p {
        "boolean" | "bool" => Value::Bool(cur.next_bool()),
        "number" | "integer" | "float" => {
            if looks_like_verbosity(hint) {
                return Value::Number((cur.next_u8() % 4).into());
            }
            if hint.contains("height") || hint == "n" || hint.contains("vout") {
                return Value::Number((cur.next_u8() as u64 % 64).into());
            }
            if hint.contains("conf_target") || hint.contains("nblocks") {
                return Value::Number((1 + (cur.next_u8() % 25) as u64).into());
            }
            let n = cur.next_i64().saturating_abs() % 10_000;
            Value::Number(n.into())
        }
        "amount" => {
            let n = (cur.next_u8() as u64) % 1000;
            Value::Number(n.into())
        }
        "null" | "none" => Value::Null,
        "hex" => {
            let nbytes = if looks_like_blockhash(hint) || looks_like_txid(hint) {
                32
            } else if looks_like_hex_hash(hint, ty) {
                if cur.next_bool() {
                    32
                } else {
                    cur.next_usize(33).max(1)
                }
            } else {
                cur.next_usize(33)
            };
            generate_hex(cur, nbytes)
        }
        "string" if looks_like_blockhash(hint) || looks_like_txid(hint) => generate_hex(cur, 32),
        _ => Value::String(cur.next_string(12)),
    }
}

/// Classify a successful or failed RPC outcome against `rpc.result`.
///
/// `raw_decode`: optional second channel — `Err` = Raw serde failed after IR ok.
pub fn classify(
    rpc: &RpcDef,
    outcome: Result<Value, InvokeError>,
    raw_decode: Option<Result<(), String>>,
) -> OracleClass {
    match outcome {
        Err(InvokeError::Transport(detail)) => OracleClass::TransportError { detail },
        Err(InvokeError::Rpc { code, message }) => OracleClass::ExpectedReject { code, message },
        Ok(value) => {
            if let Some(result_ty) = rpc.result.as_ref() {
                if let Err(detail) = validate_json_matches_type(result_ty, &value) {
                    return OracleClass::SchemaMismatch { detail };
                }
            }
            if let Some(Err(detail)) = raw_decode {
                return OracleClass::DecodeFail { detail };
            }
            OracleClass::Ok
        }
    }
}

/// Generate params, invoke, classify.
pub fn run_oracle_case(
    rpc: &RpcDef,
    data: &[u8],
    invoker: &mut dyn RpcInvoker,
    raw_decode: Option<Result<(), String>>,
) -> OracleReport {
    let params = generate_params(rpc, data);
    let outcome = invoker.invoke(&rpc.name, &params);
    let class = classify(rpc, outcome, raw_decode);
    OracleReport { method: rpc.name.clone(), params, class }
}

/// Look up an RPC by name in IR.
pub fn find_rpc<'a>(ir: &'a ProtocolIR, name: &str) -> Option<&'a RpcDef> {
    ir.get_rpc_methods().into_iter().find(|r| r.name == name)
}

/// Allowlisted methods that are safe for oracle smoke/fuzz (no wallet, no destructive).
///
/// Includes zero-arg probes plus read-only param methods so generators get exercised.
pub fn default_allowlist() -> &'static [&'static str] {
    &[
        // zero-arg probes
        "getblockchaininfo",
        "getblockcount",
        "getconnectioncount",
        "getdifficulty",
        "getbestblockhash",
        "getnetworkinfo",
        "getrpcinfo",
        "uptime",
        "help",
        "getmininginfo",
        "getmempoolinfo",
        "getpeerinfo",
        "listbanned",
        "getaddrmaninfo",
        // param-taking (expect many ExpectedReject on empty chain; still oracle-useful)
        "getblockhash",
        "getblock",
        "getblockheader",
        "getrawmempool",
        "getnetworkhashps",
        "estimatesmartfee",
        "validateaddress",
        "getdescriptorinfo",
        "gettxout",
        "decoderawtransaction",
    ]
}

/// Pick a method from allowlist using fuzz bytes; falls back to first allowlisted present in IR.
pub fn pick_rpc<'a>(ir: &'a ProtocolIR, data: &[u8], allowlist: &[&str]) -> Option<&'a RpcDef> {
    let available: Vec<&RpcDef> = allowlist.iter().filter_map(|n| find_rpc(ir, n)).collect();
    if available.is_empty() {
        return None;
    }
    let mut cur = ByteCursor::new(data);
    let idx = cur.next_usize(available.len());
    Some(available[idx])
}

/// JSON object for a corpus finding (method, params, class, detail, seed_hex).
pub fn report_to_finding(report: &OracleReport, seed: &[u8]) -> Value {
    let (class, detail) = match &report.class {
        OracleClass::Ok => ("ok", Value::Null),
        OracleClass::ExpectedReject { code, message } =>
            ("expected_reject", serde_json::json!({ "code": code, "message": message })),
        OracleClass::SchemaMismatch { detail } =>
            ("schema_mismatch", Value::String(detail.clone())),
        OracleClass::DecodeFail { detail } => ("decode_fail", Value::String(detail.clone())),
        OracleClass::TransportError { detail } =>
            ("transport_error", Value::String(detail.clone())),
    };
    serde_json::json!({
        "method": report.method,
        "params": report.params,
        "class": class,
        "detail": detail,
        "seed_hex": seed.iter().map(|b| format!("{b:02x}")).collect::<String>(),
    })
}

/// Stable basename for writing a finding under a corpus directory.
pub fn finding_basename(report: &OracleReport) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(report.method.as_bytes());
    if let Ok(bytes) = serde_json::to_vec(&report.params) {
        hasher.update(&bytes);
    }
    let class = match &report.class {
        OracleClass::Ok => "ok",
        OracleClass::ExpectedReject { .. } => "reject",
        OracleClass::SchemaMismatch { .. } => "mismatch",
        OracleClass::DecodeFail { .. } => "decode",
        OracleClass::TransportError { .. } => "transport",
    };
    let digest = hasher.finalize();
    let short = digest.iter().take(8).map(|b| format!("{b:02x}")).collect::<String>();
    format!("{}_{}_{}.json", report.method, class, short)
}

/// Summarize a batch of reports (counts by class).
pub fn summarize(reports: &[OracleReport]) -> BTreeMap<&'static str, usize> {
    let mut map = BTreeMap::new();
    for r in reports {
        let key: &'static str = match &r.class {
            OracleClass::Ok => "ok",
            OracleClass::ExpectedReject { .. } => "expected_reject",
            OracleClass::SchemaMismatch { .. } => "schema_mismatch",
            OracleClass::DecodeFail { .. } => "decode_fail",
            OracleClass::TransportError { .. } => "transport_error",
        };
        *map.entry(key).or_insert(0) += 1;
    }
    map
}

#[cfg(test)]
mod tests {
    use ir::ParamDef;
    use serde_json::json;

    use super::*;

    struct MockInvoker {
        response: Result<Value, InvokeError>,
    }

    impl RpcInvoker for MockInvoker {
        fn invoke(&mut self, _method: &str, _params: &[Value]) -> Result<Value, InvokeError> {
            self.response.clone()
        }
    }

    fn primitive_number_rpc() -> RpcDef {
        RpcDef {
            name: "getdifficulty".to_string(),
            description: "test".to_string(),
            params: vec![],
            result: Some(TypeDef {
                name: "number".to_string(),
                description: String::new(),
                kind: TypeKind::Primitive,
                fields: None,
                variants: None,
                union_variants: None,
                base_type: None,
                protocol_type: Some("number".to_string()),
                canonical_name: None,
                type_identity: None,
                condition: None,
                map_value: None,
                map_key_protocol_type: None,
            }),
            category: "blockchain".to_string(),
            access_level: Default::default(),
            requires_private_keys: false,
            version_added: None,
            version_removed: None,
            examples: None,
            hidden: None,
            result_discriminator: None,
        }
    }

    #[test]
    fn classify_ok_number() {
        let rpc = primitive_number_rpc();
        let class = classify(&rpc, Ok(json!(1.5)), None);
        assert_eq!(class, OracleClass::Ok);
    }

    #[test]
    fn classify_schema_mismatch_on_wrong_shape() {
        let rpc = primitive_number_rpc();
        let class = classify(&rpc, Ok(json!("not-a-number")), None);
        assert!(matches!(class, OracleClass::SchemaMismatch { .. }));
        assert!(class.is_oracle_finding());
    }

    #[test]
    fn classify_expected_reject() {
        let rpc = primitive_number_rpc();
        let class = classify(
            &rpc,
            Err(InvokeError::Rpc { code: Some(-8), message: "invalid".into() }),
            None,
        );
        assert_eq!(
            class,
            OracleClass::ExpectedReject { code: Some(-8), message: "invalid".into() }
        );
    }

    #[test]
    fn parse_transport_rpc_body() {
        let err = invoke_error_from_rpc_body(
            r#"{"code":-5,"message":"No such mempool or blockchain transaction"}"#,
        );
        match err {
            InvokeError::Rpc { code: Some(-5), message } => {
                assert!(message.contains("mempool") || message.contains("transaction"));
            }
            other => panic!("expected Rpc, got {other:?}"),
        }
    }

    #[test]
    fn classify_decode_fail_after_ir_ok() {
        let rpc = primitive_number_rpc();
        let class = classify(&rpc, Ok(json!(2)), Some(Err("serde boom".into())));
        assert_eq!(class, OracleClass::DecodeFail { detail: "serde boom".into() });
    }

    #[test]
    fn run_oracle_case_with_mock() {
        let rpc = primitive_number_rpc();
        let mut inv = MockInvoker { response: Ok(json!(42)) };
        let report = run_oracle_case(&rpc, b"\0\0\0", &mut inv, None);
        assert_eq!(report.method, "getdifficulty");
        assert_eq!(report.class, OracleClass::Ok);
    }

    #[test]
    fn generate_params_respects_required() {
        let mut rpc = primitive_number_rpc();
        rpc.name = "getblockhash".into();
        rpc.params = vec![ParamDef {
            name: "height".into(),
            param_type: TypeDef {
                name: "number".into(),
                description: String::new(),
                kind: TypeKind::Primitive,
                fields: None,
                variants: None,
                union_variants: None,
                base_type: None,
                protocol_type: Some("integer".into()),
                canonical_name: None,
                type_identity: None,
                condition: None,
                map_value: None,
                map_key_protocol_type: None,
            },
            required: true,
            description: String::new(),
            default_value: None,
            version_added: None,
            version_removed: None,
        }];
        let params = generate_params(&rpc, b"\x01\x02\x03\x04\x05\x06\x07\x08");
        assert_eq!(params.len(), 1);
        assert!(params[0].is_number());
    }

    #[test]
    fn blockhash_param_is_64_hex() {
        let mut rpc = primitive_number_rpc();
        rpc.name = "getblock".into();
        rpc.params = vec![ParamDef {
            name: "blockhash".into(),
            param_type: TypeDef {
                name: "string".into(),
                description: String::new(),
                kind: TypeKind::Primitive,
                fields: None,
                variants: None,
                union_variants: None,
                base_type: None,
                protocol_type: Some("hex".into()),
                canonical_name: None,
                type_identity: None,
                condition: None,
                map_value: None,
                map_key_protocol_type: None,
            },
            required: true,
            description: String::new(),
            default_value: None,
            version_added: None,
            version_removed: None,
        }];
        let params = generate_params(&rpc, &[0u8; 64]);
        assert_eq!(params.len(), 1);
        let s = params[0].as_str().expect("hex string");
        assert_eq!(s.len(), 64, "blockhash should be 32-byte hex");
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn finding_basename_stable() {
        let report = OracleReport {
            method: "getdifficulty".into(),
            params: vec![],
            class: OracleClass::SchemaMismatch { detail: "x".into() },
        };
        let a = finding_basename(&report);
        let b = finding_basename(&report);
        assert_eq!(a, b);
        assert!(a.contains("mismatch"));
        let finding = report_to_finding(&report, b"abc");
        assert_eq!(finding["class"], "schema_mismatch");
        assert_eq!(finding["seed_hex"], "616263");
    }
}
