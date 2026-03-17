//! Version-specific response type generator
//!
//! This module enhances the response type generator to use version-specific
//! type metadata extracted from corepc to generate accurate types for each
//! Bitcoin Core version.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};

use ir::{ProtocolIR, RpcDef, TypeDef, TypeKind};
use types::{Implementation, ProtocolVersion};

use super::doc_comment::{write_doc_comment, write_doc_line};
use crate::Result;

// Type alias to reduce type complexity
type SymbolRecorder = fn(&str, &str);

// Safe global to record external symbol usage via a callback
static EXTERNAL_SYMBOL_RECORDER: OnceLock<Mutex<Option<SymbolRecorder>>> = OnceLock::new();

/// Provide a recorder callback for external symbols
pub fn set_external_symbol_recorder(recorder: SymbolRecorder) {
    let slot = EXTERNAL_SYMBOL_RECORDER.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().expect("recorder mutex poisoned");
    *guard = Some(recorder);
}

fn record_external_symbol(crate_name: &str, symbol: &str) {
    if let Some(slot) = EXTERNAL_SYMBOL_RECORDER.get() {
        if let Some(rec) = *slot.lock().expect("recorder mutex poisoned") {
            rec(crate_name, symbol);
        }
    }
}

/// Public wrapper for other generators to record symbol usage without importing private helpers
pub fn record_external_symbol_usage(crate_name: &str, symbol: &str) {
    record_external_symbol(crate_name, symbol);
}

/// Enhanced response type generator that uses version-specific metadata
pub struct VersionSpecificResponseTypeGenerator {
    version: ProtocolVersion,
    implementation: String,
}

impl VersionSpecificResponseTypeGenerator {
    /// Create a new version-specific response type generator
    pub fn new(version: ProtocolVersion, implementation: String) -> Self {
        Self { version, implementation }
    }

    /// Create a new version-specific response type generator from IR
    pub fn from_ir(
        version: ProtocolVersion,
        implementation: String,
        _ir: &ProtocolIR,
    ) -> Result<Self> {
        // Simplified - IR doesn't track per-type versions, so just use version
        Ok(Self { version, implementation })
    }

    /// Filter a `TypeDef`'s fields (and nested types) by `version_added` / `version_removed`
    /// for the generator's target version. Delegates to the openrpc adapter for Bitcoin Core;
    /// other implementations use the IR as-is.
    fn filter_type_def_for_version(&self, ty: &ir::TypeDef) -> ir::TypeDef {
        if self.implementation != "bitcoin_core" {
            return ty.clone();
        }
        adapters::bitcoin_core::openrpc::filter_type_def_for_version(ty, self.version.as_str())
    }

    /// Generate version-specific response types
    pub fn generate(&self, methods: &[RpcDef]) -> Result<Vec<(String, String)>> {
        let mut out = String::from("//! Generated version-specific RPC response types\n");
        out.push_str("//! \n");
        let implementation_display = Implementation::from_str(&self.implementation)
            .map(|impl_| impl_.display_name().to_string())
            .unwrap_or_else(|_| self.implementation.clone());
        out.push_str(&format!(
            "//! Generated for {} {}\n",
            implementation_display,
            self.version.short()
        ));
        out.push_str("//! \n");
        out.push_str("//! These types are version-specific and may not match other versions.\n");

        // First, collect all nested types that need to be generated
        // BTreeSet for deterministic iteration order so generated output is stable.
        let mut nested_types = BTreeSet::new();
        for method in methods {
            if let Some(result) = &method.result {
                self.collect_nested_types_from_type_def(result, &mut nested_types);
            }
        }

        // Add imports
        out.push_str("use serde::{Deserialize, Serialize};\n");
        out.push_str("use std::str::FromStr;\n");

        // Add conditional imports based on what types are used
        let mut needs_btreemap = false;
        let mut needs_transaction = false;
        let mut needs_txout = false;
        let mut needs_scriptbuf = false;
        let mut needs_keysource = false;
        let mut needs_taptree = false;
        let mut needs_proprietarykey = false;
        let mut needs_hashmap = false;
        let mut needs_amount_deserializer = false;

        // Check all methods for type usage - generate directly from IR
        for method in methods {
            if let Some(result) = &method.result {
                let result = self.filter_type_def_for_version(result);
                if let Some(fields) = &result.fields {
                    for field in fields {
                        let rust_type = Self::response_field_type_override(
                            method.name.as_str(),
                            &field.key.as_ident(),
                        )
                        .map(String::from)
                        .unwrap_or_else(|| {
                            self.map_ir_type_to_rust(&field.field_type, &field.key.as_ident())
                        });
                        let field_type = &field.field_type.name;
                        if field_type.contains("BTreeMap") || rust_type.contains("BTreeMap") {
                            needs_btreemap = true;
                        }
                        if field_type.contains("HashMap") || rust_type.contains("HashMap") {
                            needs_hashmap = true;
                        }
                        if field_type.contains("KeySource") {
                            needs_keysource = true;
                        }
                        if field_type.contains("ScriptBuf") {
                            needs_scriptbuf = true;
                        }
                        if field_type.contains("Transaction") {
                            needs_transaction = true;
                        }
                        if field_type.contains("TxOut") {
                            needs_txout = true;
                        }
                        if field_type.contains("TapTree") {
                            needs_taptree = true;
                        }
                        if field_type.contains("ProprietaryKey") {
                            needs_proprietarykey = true;
                        }
                        // Check if field type is bitcoin::Amount
                        if rust_type == "bitcoin::Amount" || rust_type.contains("bitcoin::Amount") {
                            needs_amount_deserializer = true;
                        }
                    }
                }
            }
        }

        if needs_btreemap {
            out.push_str("use std::collections::BTreeMap;\n");
        }
        if needs_hashmap {
            out.push_str("use std::collections::HashMap;\n");
        }
        if needs_keysource {
            out.push_str("use bitcoin::bip32::KeySource;\n");
            record_external_symbol("bitcoin", "bip32::KeySource");
        }
        if needs_proprietarykey {
            out.push_str("use bitcoin::psbt::raw::ProprietaryKey;\n");
            record_external_symbol("bitcoin", "psbt::raw::ProprietaryKey");
        }
        if needs_scriptbuf {
            out.push_str("use bitcoin::ScriptBuf;\n");
            record_external_symbol("bitcoin", "ScriptBuf");
        }
        if needs_taptree {
            out.push_str("use bitcoin::taproot::TapTree;\n");
            record_external_symbol("bitcoin", "taproot::TapTree");
        }
        if needs_transaction {
            out.push_str("use bitcoin::Transaction;\n");
            record_external_symbol("bitcoin", "Transaction");
        }
        if needs_txout {
            out.push_str("use bitcoin::TxOut;\n");
            record_external_symbol("bitcoin", "TxOut");
        }

        out.push('\n');

        let type_registry = Self::build_type_registry(methods);

        // Manually emitted response structs (decoded-tx helpers, GetBlockTemplateTransaction, etc.)
        // are emitted by dedicated helpers (for example, emit_decoded_tx_types) and not from IR.
        let mut processed_types = BTreeSet::new();
        for name in Self::MANUAL_RESPONSE_TYPE_NAMES {
            processed_types.insert((*name).to_string());
        }

        // Generate nested types first, recursively collecting more nested types.
        // BTreeSet gives deterministic iteration over type names.
        let mut all_nested_types = nested_types.clone();

        while !all_nested_types.is_empty() {
            let current_types: Vec<String> = all_nested_types.iter().cloned().collect();
            all_nested_types.clear();

            for nested_type in &current_types {
                if processed_types.contains(nested_type) {
                    continue;
                }

                if let Some(nested_struct) =
                    self.generate_nested_type(nested_type, &type_registry)?
                {
                    out.push_str(&nested_struct);
                    out.push('\n');
                    processed_types.insert(nested_type.clone());

                    // Collect nested types - simplified since IR doesn't track per-type versions
                    // Nested types will be discovered when generating from IR result types
                } else {
                    processed_types.insert(nested_type.clone());
                }
            }
        }

        // Emit decoded tx types once so they are defined before any method response that references them.
        let mut decoded_tx_buf = String::new();
        let fee_required = methods
            .iter()
            .find(|m| m.name == "getblock")
            .and_then(|method| Self::get_getblock_decoded_tx_element_type(method))
            .and_then(|ty| Self::find_field_in_type(ty, "fee"))
            .map(|f| f.required);
        self.emit_decoded_tx_types(
            &mut decoded_tx_buf,
            fee_required,
            type_registry.contains_key("DecodedScriptPubKey"),
        )?;
        if !decoded_tx_buf.is_empty() {
            out.push_str(&decoded_tx_buf);
            out.push_str("\n\n");
        }

        // Emit GetBlockTemplateTransaction when getblocktemplate is present so GetBlockTemplateResponse can use it.
        if methods.iter().any(|m| m.name == "getblocktemplate") {
            let mut gbt_tx_buf = String::new();
            self.emit_get_block_template_transaction(&mut gbt_tx_buf)?;
            out.push_str(&gbt_tx_buf);
            out.push_str("\n");
        }

        // Generate response structs for each method
        let mut has_any_responses = false;
        for method in methods {
            if let Some(response_struct) = self.generate_method_response(method)? {
                if !has_any_responses {
                    has_any_responses = true;
                }
                out.push_str(&response_struct);
                out.push('\n');
            }
        }

        // Add amount deserializer helper functions if needed
        if needs_amount_deserializer {
            // Deserializer for non-optional Amount fields
            out.push_str("\n/// Deserializer for bitcoin::Amount that handles both float (BTC) and integer (satoshis) formats\n");
            out.push_str("/// Bitcoin Core returns amounts as floats in BTC, but some fields may be integers in satoshis\n");
            out.push_str("fn amount_from_btc_float<'de, D>(deserializer: D) -> Result<bitcoin::Amount, D::Error>\n");
            out.push_str("where\n");
            out.push_str("    D: serde::Deserializer<'de>,\n");
            out.push_str("{\n");
            out.push_str("    use serde::de::{self, Visitor};\n");
            out.push_str("    use std::fmt;\n");
            out.push_str("\n");
            out.push_str("    struct AmountVisitor;\n");
            out.push_str("\n");
            out.push_str("    impl Visitor<'_> for AmountVisitor {\n");
            out.push_str("        type Value = bitcoin::Amount;\n");
            out.push_str("\n");
            out.push_str(
                "        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {\n",
            );
            out.push_str(
                "            formatter.write_str(\"a number (float BTC or integer satoshis)\")\n",
            );
            out.push_str("        }\n");
            out.push_str("\n");
            out.push_str("        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>\n");
            out.push_str("        where\n");
            out.push_str("            E: de::Error,\n");
            out.push_str("        {\n");
            out.push_str("            bitcoin::Amount::from_btc(v).map_err(|e| E::custom(format!(\"Invalid BTC amount: {}\", e)))\n");
            out.push_str("        }\n");
            out.push_str("\n");
            out.push_str("        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>\n");
            out.push_str("        where\n");
            out.push_str("            E: de::Error,\n");
            out.push_str("        {\n");
            out.push_str("            Ok(bitcoin::Amount::from_sat(v))\n");
            out.push_str("        }\n");
            out.push_str("\n");
            out.push_str("        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>\n");
            out.push_str("        where\n");
            out.push_str("            E: de::Error,\n");
            out.push_str("        {\n");
            out.push_str("            if v < 0 {\n");
            out.push_str("                return Err(E::custom(format!(\"Amount cannot be negative: {}\", v)));\n");
            out.push_str("            }\n");
            out.push_str("            Ok(bitcoin::Amount::from_sat(v as u64))\n");
            out.push_str("        }\n");
            out.push_str("    }\n");
            out.push_str("\n");
            out.push_str("    deserializer.deserialize_any(AmountVisitor)\n");
            out.push_str("}\n");

            // Deserializer for optional Amount fields
            out.push_str("\n/// Deserializer for Option<bitcoin::Amount> that handles both float (BTC) and integer (satoshis) formats\n");
            out.push_str("/// Bitcoin Core returns amounts as floats in BTC, but some fields may be integers in satoshis\n");
            out.push_str("/// This deserializer also handles null/None values\n");
            out.push_str("fn option_amount_from_btc_float<'de, D>(deserializer: D) -> Result<Option<bitcoin::Amount>, D::Error>\n");
            out.push_str("where\n");
            out.push_str("    D: serde::Deserializer<'de>,\n");
            out.push_str("{\n");
            out.push_str("    use serde::de::{self, Visitor};\n");
            out.push_str("    use std::fmt;\n");
            out.push_str("\n");
            out.push_str("    struct OptionAmountVisitor;\n");
            out.push_str("\n");
            out.push_str("    #[allow(clippy::needless_lifetimes)]\n");
            out.push_str("    impl<'de> Visitor<'de> for OptionAmountVisitor {\n");
            out.push_str("        type Value = Option<bitcoin::Amount>;\n");
            out.push_str("\n");
            out.push_str(
                "        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {\n",
            );
            out.push_str("            formatter.write_str(\"an optional number (float BTC or integer satoshis)\")\n");
            out.push_str("        }\n");
            out.push_str("\n");
            out.push_str("        fn visit_none<E>(self) -> Result<Self::Value, E>\n");
            out.push_str("        where\n");
            out.push_str("            E: de::Error,\n");
            out.push_str("        {\n");
            out.push_str("            Ok(None)\n");
            out.push_str("        }\n");
            out.push_str("\n");
            out.push_str("        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>\n");
            out.push_str("        where\n");
            out.push_str("            D: serde::Deserializer<'de>,\n");
            out.push_str("        {\n");
            out.push_str("            amount_from_btc_float(deserializer).map(Some)\n");
            out.push_str("        }\n");
            out.push_str("\n");
            out.push_str("        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>\n");
            out.push_str("        where\n");
            out.push_str("            E: de::Error,\n");
            out.push_str("        {\n");
            out.push_str("            bitcoin::Amount::from_btc(v)\n");
            out.push_str(
                "                .map_err(|e| E::custom(format!(\"Invalid BTC amount: {}\", e)))\n",
            );
            out.push_str("                .map(Some)\n");
            out.push_str("        }\n");
            out.push_str("\n");
            out.push_str("        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>\n");
            out.push_str("        where\n");
            out.push_str("            E: de::Error,\n");
            out.push_str("        {\n");
            out.push_str("            Ok(Some(bitcoin::Amount::from_sat(v)))\n");
            out.push_str("        }\n");
            out.push_str("\n");
            out.push_str("        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>\n");
            out.push_str("        where\n");
            out.push_str("            E: de::Error,\n");
            out.push_str("        {\n");
            out.push_str("            if v < 0 {\n");
            out.push_str("                return Err(E::custom(format!(\"Amount cannot be negative: {}\", v)));\n");
            out.push_str("            }\n");
            out.push_str("            Ok(Some(bitcoin::Amount::from_sat(v as u64)))\n");
            out.push_str("        }\n");
            out.push_str("    }\n");
            out.push_str("\n");
            out.push_str("    deserializer.deserialize_any(OptionAmountVisitor)\n");
            out.push_str("}\n");
        }

        let filename = "responses.rs".to_string();
        Ok(vec![(filename, out)])
    }

    /// Build a registry of named object types by walking all method result types recursively.
    /// First occurrence of each type name wins. Used to generate structs from IR (e.g. DecodedScriptPubKey).
    /// BTreeMap so keys are iterated in stable, sorted order.
    fn build_type_registry(methods: &[RpcDef]) -> BTreeMap<String, TypeDef> {
        let mut reg = BTreeMap::new();
        fn visit(ty: &TypeDef, reg: &mut BTreeMap<String, TypeDef>) {
            if matches!(ty.kind, TypeKind::Object) && ty.name != "object" && ty.name != "array" {
                reg.entry(ty.name.clone()).or_insert_with(|| ty.clone());
            }
            if let Some(fields) = &ty.fields {
                for f in fields {
                    visit(&f.field_type, reg);
                }
            }
        }
        for method in methods {
            if let Some(ref result) = method.result {
                visit(result, &mut reg);
            }
        }
        reg
    }

    /// If this result represents a top-level JSON array in IR, return the element type.
    ///
    /// Delegates to `TypeDef::array_element_type()` so that array semantics are
    /// centralized in the IR layer instead of re-encoding `FieldKey` conventions here.
    fn array_element_type_from_ir(result: &ir::TypeDef) -> Option<&ir::TypeDef> {
        result.array_element_type()
    }

    /// Stronger types for specific (rpc_name, field_name) when the default would be serde_json::Value.
    fn response_field_type_override(rpc_name: &str, field_name: &str) -> Option<&'static str> {
        match (rpc_name, field_name) {
            ("getblocktemplate", "vbavailable") => Some("HashMap<String, u32>"),
            ("getblocktemplate", "coinbaseaux") => Some("HashMap<String, String>"),
            ("getblocktemplate", "transactions") => Some("Vec<GetBlockTemplateTransaction>"),
            (_, "transactionid") => Some("bitcoin::Txid"),
            _ => None,
        }
    }

    /// RPCs that return arbitrary JSON (echo/echojson); need Value wrapper with whole-response deserialize
    const ECHO_JSON_VALUE_RPCS: &[&str] = &["echo", "echojson"];

    /// Fields that bitcoind may omit; force Option<T> for these (rpc_name, field_name)
    fn optional_field_override(rpc_name: &str, field_name: &str) -> bool {
        matches!(
            (rpc_name, field_name),
            ("analyzepsbt", "fee")
                | ("decodepsbt", "fee")
                | ("getaddrmaninfo", "network")
                | ("getblocktemplate", "field_0")
                | ("getindexinfo", "name")
                | ("getrawaddrman", "table")
                | ("gettxout", "field_0")
                | ("gettxoutsetinfo", "total_unspendable_amount")
                | ("logging", "category")
        )
    }

    /// Returns true iff the IR field has protocol_type "elision". These are documentation/type placeholders,
    /// not real JSON keys.
    fn is_elision_field(field: &ir::FieldDef) -> bool {
        field.field_type.protocol_type.as_deref() == Some("elision")
    }

    /// Returns true iff the field should be skipped when emitting a struct field for the given RPC.
    /// This includes elision placeholders and method-specific scaffolding fields that are not real
    /// JSON keys in Core's responses.
    fn should_skip_field_in_struct(rpc_name: &str, field: &ir::FieldDef) -> bool {
        if Self::is_elision_field(field) {
            return true;
        }

        if rpc_name == "getblock" {
            return matches!(
                field.key.as_ident().as_str(),
                "tx_1" | "tx_2" | "field_21" | "field_23"
            );
        }

        false
    }

    /// Returns the TypeDef for one element of the getblock decoded-tx array (verbosity 2/3).
    /// That type's fields include the elision placeholder and explicit fields like `fee`.
    /// Used with [`Self::find_field_in_type`] to drive optionality from the IR without hardcoding paths in the emitter.
    fn get_getblock_decoded_tx_element_type(method: &RpcDef) -> Option<&ir::TypeDef> {
        let result = method.result.as_ref()?;
        let top_fields = result.fields.as_ref()?;
        let tx1 = top_fields.iter().find(|f| f.key.as_ident() == "tx_1")?;
        let array_wrapper = tx1.field_type.fields.as_ref()?;
        let first = array_wrapper.first()?;
        let inner_fields = first.field_type.fields.as_ref()?;
        let inner_first = inner_fields.first()?;
        Some(&inner_first.field_type)
    }

    /// Returns the field with the given name in the type's direct fields, if any.
    fn find_field_in_type<'a>(
        type_def: &'a ir::TypeDef,
        field_name: &str,
    ) -> Option<&'a ir::FieldDef> {
        type_def.fields.as_ref()?.iter().find(|f| f.key.as_ident() == field_name)
    }

    /// Emits Rust structs for decoded transaction details (getblock verbosity 2/3, getrawtransaction verbose).
    /// Does not use deny_unknown_fields on inner structs so new Core fields do not break deserialization.
    /// When `skip_decoded_script_pubkey` is true, DecodedScriptPubKey is already emitted from the IR type registry.
    /// `fee_required`: when `Some(true)` emit `fee: f64`, otherwise `fee: Option<f64>`; caller derives this from the IR.
    fn emit_decoded_tx_types(
        &self,
        buf: &mut String,
        fee_required: Option<bool>,
        skip_decoded_script_pubkey: bool,
    ) -> Result<()> {
        if !skip_decoded_script_pubkey {
            write_doc_line(buf, "Script pubkey in decoded tx output.", "")?;
            write_doc_line(
                buf,
                "See: <https://github.com/bitcoin/bitcoin/blob/744d47fcee0d32a71154292699bfdecf954a6065/src/core_io.cpp#L409-L427>",
                "",
            )?;
            writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
            writeln!(buf, "pub struct DecodedScriptPubKey {{")?;
            write_doc_comment(buf, "Script in human-readable assembly form.", "    ")?;
            writeln!(buf, "    pub asm: String,")?;
            write_doc_comment(
                buf,
                "Output script descriptor; present only when address/descriptor info was requested (include_address).",
                "    ",
            )?;
            writeln!(buf, "    #[serde(default, skip_serializing_if = \"Option::is_none\")]")?;
            writeln!(buf, "    pub desc: Option<String>,")?;
            write_doc_comment(
                buf,
                "Output script serialized as hex; present only when hex was requested (include_hex).",
                "    ",
            )?;
            writeln!(buf, "    #[serde(default, skip_serializing_if = \"Option::is_none\")]")?;
            writeln!(buf, "    pub hex: Option<String>,")?;
            write_doc_comment(
                buf,
                "Categorized script type (for example, \"pubkeyhash\").",
                "    ",
            )?;
            writeln!(buf, "    #[serde(rename = \"type\")]")?;
            writeln!(buf, "    pub type_: String,")?;
            write_doc_comment(
                buf,
                "Decoded destination address for this script, when available.",
                "    ",
            )?;
            writeln!(buf, "    #[serde(default, skip_serializing_if = \"Option::is_none\")]")?;
            writeln!(buf, "    pub address: Option<String>,")?;
            writeln!(buf, "}}")?;
            writeln!(buf)?;
        }

        write_doc_line(buf, "Script sig in decoded tx input.", "")?;
        write_doc_line(
            buf,
            "See: <https://github.com/bitcoin/bitcoin/blob/744d47fcee0d32a71154292699bfdecf954a6065/src/core_io.cpp#L458-L461>",
            "",
        )?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(buf, "pub struct DecodedScriptSig {{")?;
        write_doc_comment(buf, "scriptSig in human-readable assembly form.", "    ")?;
        writeln!(buf, "    pub asm: String,")?;
        write_doc_comment(buf, "scriptSig serialized as hex.", "    ")?;
        writeln!(buf, "    pub hex: String,")?;
        writeln!(buf, "}}")?;
        writeln!(buf)?;

        write_doc_line(
            buf,
            "Previous output (prevout) in decoded tx input; present for getblock verbosity 3.",
            "",
        )?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(buf, "pub struct DecodedPrevout {{")?;
        write_doc_comment(
            buf,
            "True if the prevout was created by a coinbase transaction.",
            "    ",
        )?;
        writeln!(buf, "    pub generated: bool,")?;
        write_doc_comment(buf, "Block height where the prevout was created.", "    ")?;
        writeln!(buf, "    pub height: i64,")?;
        write_doc_comment(buf, "Decoded script pubkey of the prevout output.", "    ")?;
        writeln!(buf, "    #[serde(rename = \"scriptPubKey\")]")?;
        writeln!(buf, "    pub script_pubkey: DecodedScriptPubKey,")?;
        write_doc_comment(buf, "Value of the prevout output in BTC.", "    ")?;
        writeln!(buf, "    pub value: f64,")?;
        writeln!(buf, "}}")?;
        writeln!(buf)?;

        write_doc_line(
            buf,
            "Transaction input in decoded tx; prevout is None for getblock verbosity 2, Some for verbosity 3.",
            "",
        )?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(buf, "pub struct DecodedVin {{")?;
        write_doc_comment(buf, "Transaction id of the previous output being spent.", "    ")?;
        writeln!(buf, "    pub txid: String,")?;
        write_doc_comment(buf, "Index of the previous output being spent.", "    ")?;
        writeln!(buf, "    pub vout: u32,")?;
        write_doc_comment(buf, "Decoded scriptSig for this input, when present.", "    ")?;
        writeln!(
            buf,
            "    #[serde(rename = \"scriptSig\", default, skip_serializing_if = \"Option::is_none\")]"
        )?;
        writeln!(buf, "    pub script_sig: Option<DecodedScriptSig>,")?;
        write_doc_comment(buf, "Input sequence number.", "    ")?;
        writeln!(buf, "    pub sequence: u64,")?;
        write_doc_comment(buf, "Witness stack items for this input (if any).", "    ")?;
        writeln!(buf, "    #[serde(rename = \"txinwitness\", default, skip_serializing_if = \"Option::is_none\")]")?;
        writeln!(buf, "    pub tx_in_witness: Option<Vec<String>>,")?;
        write_doc_comment(
            buf,
            "Decoded details of the previous output when verbosity includes prevout.",
            "    ",
        )?;
        writeln!(buf, "    #[serde(default, skip_serializing_if = \"Option::is_none\")]")?;
        writeln!(buf, "    pub prevout: Option<DecodedPrevout>,")?;
        writeln!(buf, "}}")?;
        writeln!(buf)?;

        write_doc_line(buf, "Transaction output in decoded tx; mirrors Core vout object.", "")?;
        write_doc_line(
            buf,
            "See: <https://github.com/bitcoin/bitcoin/blob/744d47fcee0d32a71154292699bfdecf954a6065/src/core_io.cpp#L495-L519>",
            "",
        )?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(buf, "pub struct DecodedVout {{")?;
        write_doc_comment(buf, "Value in BTC of this output.", "    ")?;
        writeln!(buf, "    pub value: f64,")?;
        write_doc_comment(buf, "Index of this output within the transaction.", "    ")?;
        writeln!(buf, "    pub n: u32,")?;
        write_doc_comment(buf, "Decoded script pubkey of this output.", "    ")?;
        writeln!(buf, "    #[serde(rename = \"scriptPubKey\")]")?;
        writeln!(buf, "    pub script_pubkey: DecodedScriptPubKey,")?;
        writeln!(buf, "}}")?;
        writeln!(buf)?;

        write_doc_line(
            buf,
            "Decoded transaction details (getblock verbosity 2/3 and getrawtransaction verbose).",
            "",
        )?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(buf, "pub struct DecodedTxDetails {{")?;
        write_doc_comment(buf, "Transaction id.", "    ")?;
        writeln!(buf, "    pub txid: String,")?;
        write_doc_comment(buf, "Witness transaction id (wtxid).", "    ")?;
        writeln!(buf, "    pub hash: String,")?;
        write_doc_comment(buf, "Transaction version.", "    ")?;
        writeln!(buf, "    pub version: i32,")?;
        write_doc_comment(buf, "Total serialized size of the transaction in bytes.", "    ")?;
        writeln!(buf, "    pub size: u32,")?;
        write_doc_comment(buf, "Virtual transaction size (vsize) as defined in BIP 141.", "    ")?;
        writeln!(buf, "    pub vsize: u32,")?;
        write_doc_comment(buf, "Transaction weight as defined in BIP 141.", "    ")?;
        writeln!(buf, "    pub weight: u32,")?;
        write_doc_comment(buf, "Transaction locktime.", "    ")?;
        writeln!(buf, "    pub locktime: u32,")?;
        write_doc_comment(buf, "List of transaction inputs.", "    ")?;
        writeln!(buf, "    pub vin: Vec<DecodedVin>,")?;
        write_doc_comment(buf, "List of transaction outputs.", "    ")?;
        writeln!(buf, "    pub vout: Vec<DecodedVout>,")?;
        write_doc_comment(
            buf,
            "Fee paid by the transaction, when undo data is available.",
            "    ",
        )?;
        if fee_required == Some(true) {
            writeln!(buf, "    pub fee: f64,")?;
        } else {
            // Optional when IR says required=false, or when IR path is missing (e.g. older IR)
            writeln!(buf, "    #[serde(default, skip_serializing_if = \"Option::is_none\")]")?;
            writeln!(buf, "    pub fee: Option<f64>,")?;
        }
        write_doc_comment(
            buf,
            "Raw transaction serialized as hex (consistent with getrawtransaction verbose output).",
            "    ",
        )?;
        writeln!(buf, "    pub hex: String,")?;
        writeln!(buf, "}}")?;
        writeln!(buf)?;

        // Thin wrappers used by higher-level helpers around getblock verbosities.
        write_doc_line(buf, "Hex-encoded block data returned by getblock with verbosity 0.", "")?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(buf, "pub struct GetBlockV0 {{")?;
        write_doc_comment(buf, "Serialized block as a hex string.", "    ")?;
        writeln!(buf, "    pub hex: String,")?;
        writeln!(buf, "}}")?;
        writeln!(buf)?;

        write_doc_line(
            buf,
            "Verbose block view with decoded transactions (built from getblock verbosities 1 and 2).",
            "",
        )?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Serialize)]")?;
        writeln!(buf, "pub struct GetBlockWithTxsResponse {{")?;
        write_doc_comment(
            buf,
            "Block header and summary information from getblock verbosity 1.",
            "    ",
        )?;
        writeln!(buf, "    pub base: GetBlockResponse,")?;
        write_doc_comment(
            buf,
            "Fully decoded transactions in the block, matching getblock verbosity 2.",
            "    ",
        )?;
        writeln!(buf, "    pub decoded_txs: Vec<DecodedTxDetails>,")?;
        writeln!(buf, "}}")?;
        writeln!(buf)?;

        write_doc_line(
            buf,
            "Verbose block view with decoded transactions and prevout metadata (getblock verbosity 3).",
            "",
        )?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Serialize)]")?;
        writeln!(buf, "pub struct GetBlockWithPrevoutResponse {{")?;
        write_doc_comment(
            buf,
            "Verbose block view with prevout-rich inputs; wraps the verbosity-2 representation.",
            "    ",
        )?;
        writeln!(buf, "    pub inner: GetBlockWithTxsResponse,")?;
        writeln!(buf, "}}")?;
        writeln!(buf)?;

        Ok(())
    }

    /// Emit GetBlockTemplateTransaction struct for getblocktemplate response "transactions" array.
    /// BIP 22/23/145: data, depends, fee (optional), hash, sigops (optional), txid, weight.
    fn emit_get_block_template_transaction(&self, buf: &mut String) -> Result<()> {
        write_doc_line(
            buf,
            "One transaction entry in getblocktemplate \"transactions\" array (BIP 22/23/145).",
            "",
        )?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(buf, "pub struct GetBlockTemplateTransaction {{")?;
        write_doc_comment(buf, "Transaction data encoded in hexadecimal (byte-for-byte).", "    ")?;
        writeln!(buf, "    pub data: String,")?;
        write_doc_comment(
            buf,
            "1-based indexes of transactions in the 'transactions' list that must be present before this one.",
            "    ",
        )?;
        writeln!(buf, "    pub depends: Vec<i64>,")?;
        write_doc_comment(
            buf,
            "Difference in value between inputs and outputs (satoshis); absent when unknown.",
            "    ",
        )?;
        writeln!(buf, "    #[serde(default, skip_serializing_if = \"Option::is_none\")]")?;
        writeln!(buf, "    pub fee: Option<i64>,")?;
        write_doc_comment(
            buf,
            "Transaction hash including witness data (byte-reversed hex).",
            "    ",
        )?;
        writeln!(buf, "    pub hash: String,")?;
        write_doc_comment(buf, "Total SigOps cost for block limits; absent when unknown.", "    ")?;
        writeln!(buf, "    #[serde(default, skip_serializing_if = \"Option::is_none\")]")?;
        writeln!(buf, "    pub sigops: Option<i64>,")?;
        write_doc_comment(
            buf,
            "Transaction hash excluding witness data (byte-reversed hex).",
            "    ",
        )?;
        writeln!(buf, "    pub txid: String,")?;
        write_doc_comment(buf, "Total transaction weight for block limits.", "    ")?;
        writeln!(buf, "    pub weight: i64,")?;
        writeln!(buf, "}}")?;
        writeln!(buf)?;
        Ok(())
    }

    /// Generate response type for a specific method
    fn generate_method_response(&self, method: &RpcDef) -> Result<Option<String>> {
        // RPCs whose IR result is a top-level array: generate array wrapper instead of struct.
        if let Some(result) = &method.result {
            if Self::array_element_type_from_ir(result).is_some() {
                let struct_name = self.response_struct_name(method);
                return Ok(Some(self.generate_array_wrapper(method, &struct_name, result)?));
            }
        }

        // RPCs that return arbitrary JSON (echo/echojson)
        if Self::ECHO_JSON_VALUE_RPCS.contains(&method.name.as_str()) {
            let struct_name = self.response_struct_name(method);
            return Ok(Some(self.generate_value_wrapper(method, &struct_name)?));
        }

        // help returns a plain string
        if method.name.as_str() == "help" {
            let struct_name = self.response_struct_name(method);
            return Ok(Some(self.generate_primitive_wrapper(&struct_name, "string", &None)?));
        }

        // Simplified - always generate from IR data since we removed metadata
        // registries. For Bitcoin Core, we first filter the IR result type
        // using version metadata so fields that are not present in this
        // release are omitted from the generated struct.
        if let Some(result) = &method.result {
            let result = self.filter_type_def_for_version(result);
            if let Some(fields) = &result.fields {
                if !fields.is_empty() {
                    return Ok(Some(self.generate_from_ir_data(method, &result)?));
                }
            }
        }

        // For methods without result types, generate unit structs
        self.generate_unit_response(method)
    }

    /// Generate response type from IR data (TypeDef.fields). The caller must pass a result
    /// already filtered for the generator's target version.
    fn generate_from_ir_data(&self, method: &RpcDef, result: &ir::TypeDef) -> Result<String> {
        let struct_name = self.response_struct_name(method);
        let mut buf = String::new();

        // Generate struct documentation using PascalCase canonical method name
        let canonical_name =
            crate::utils::canonical_from_adapter_method(&self.implementation, &method.name, None)
                .unwrap_or_else(|_| struct_name.replace("Response", ""));
        write_doc_line(&mut buf, &format!("Response for the `{}` RPC method", canonical_name), "")?;
        if !result.description.is_empty() {
            // Add a separating blank doc line only when we have extra description,
            // so we don't emit a standalone hanging `///`.
            writeln!(&mut buf, "///")?;
            write_doc_comment(&mut buf, &result.description, "")?;
        }

        // Check if this method has conditional results (can return either string or object)
        // This happens when we have both simple type results and object results.
        // getblockstats has all-optional fields but returns only an object (no string variant);
        // use standard Deserialize so we don't require a custom visitor.
        let has_conditional_results = (method.name == "getrawtransaction"
            || self.check_conditional_results(result))
            && method.name != "getblockstats";

        if has_conditional_results {
            // Generate struct without Deserialize derive (we'll implement it manually)
            writeln!(&mut buf, "#[derive(Debug, Clone, PartialEq, Serialize)]")?;
        } else {
            // Generate struct with standard deserializer
            writeln!(&mut buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
            writeln!(
                &mut buf,
                "#[cfg_attr(feature = \"serde-deny-unknown-fields\", serde(deny_unknown_fields))]"
            )?;
        }
        writeln!(&mut buf, "pub struct {} {{", struct_name)?;

        // Generate fields from IR data (when conditional, all fields must be Option for string|object)
        if let Some(fields) = &result.fields {
            for field in fields
                .iter()
                .filter(|f| !Self::should_skip_field_in_struct(method.name.as_str(), f))
            {
                self.generate_ir_field(
                    &mut buf,
                    field,
                    &struct_name,
                    method.name.as_str(),
                    has_conditional_results,
                )?;
            }
        }

        writeln!(&mut buf, "}}")?;

        // Generate custom Deserialize implementation if needed
        if has_conditional_results {
            self.generate_conditional_deserialize_impl(&mut buf, result, &struct_name)?;
        }

        Ok(buf)
    }

    /// Emit a single struct from an IR `TypeDef` (for nested/named types from
    /// the type registry). Uses the same field logic as method responses; no
    /// `deny_unknown_fields` on inner structs. For Bitcoin Core the type is
    /// first filtered for the generator's target version so nested helpers do
    /// not include fields that are not present in this release.
    fn generate_struct_from_type_def(&self, type_def: &TypeDef, buf: &mut String) -> Result<()> {
        let type_def = self.filter_type_def_for_version(type_def);
        if !type_def.description.is_empty() {
            write_doc_comment(buf, &type_def.description, "")?;
        }
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(buf, "pub struct {} {{", type_def.name)?;
        if let Some(fields) = &type_def.fields {
            for field in fields.iter().filter(|f| !Self::is_elision_field(f)) {
                self.generate_ir_field(buf, field, &type_def.name, "", false)?;
            }
        }
        writeln!(buf, "}}")?;
        writeln!(buf)?;
        Ok(())
    }

    /// Generate a field from IR FieldDef
    fn generate_ir_field(
        &self,
        buf: &mut String,
        field: &ir::FieldDef,
        _struct_name: &str,
        rpc_name: &str,
        force_optional_conditional: bool,
    ) -> Result<()> {
        // Generate field documentation
        if !field.description.is_empty() {
            write_doc_comment(buf, &field.description, "    ")?;
        }

        // Generate field definition (use stronger type override when set)
        let base_field_type = Self::response_field_type_override(rpc_name, &field.key.as_ident())
            .map(String::from)
            .unwrap_or_else(|| self.map_ir_type_to_rust(&field.field_type, &field.key.as_ident()));
        if base_field_type.starts_with("bitcoin::") {
            let symbol = base_field_type.split("::").last().unwrap_or(&base_field_type);
            record_external_symbol("bitcoin", symbol);
        }
        let field_name = self.sanitize_identifier(&field.key.as_ident());
        let mut field_type = if field.required && !force_optional_conditional {
            base_field_type.clone()
        } else {
            format!("Option<{}>", base_field_type)
        };
        // Elision fields are type/documentation placeholders (e.g. getblock verbosity); never present as JSON keys
        if field.field_type.protocol_type.as_deref() == Some("elision") {
            field_type = format!("Option<{}>", base_field_type);
        }
        // Override: some Core fields are absent on certain networks/versions
        if field.key.as_ident() == "blockmintxfee"
            || field.key.as_ident() == "maxdatacarriersize"
            || field.key.as_ident() == "permitbaremultisig"
            || field.key.as_ident() == "limitclustercount"
            || field.key.as_ident() == "limitclustersize"
        {
            field_type = format!("Option<{}>", base_field_type);
        }
        // Override: bitcoind omits these fields in some modes/versions
        if Self::optional_field_override(rpc_name, &field.key.as_ident()) {
            field_type = format!("Option<{}>", base_field_type);
        }

        // Add serde rename attribute if the field name was changed
        if field_name != field.key.as_ident() {
            writeln!(buf, "    #[serde(rename = \"{}\")]", field.key.as_ident())?;
        }

        // When we force Option for omitted fields, allow missing key to deserialize as None
        if Self::optional_field_override(rpc_name, &field.key.as_ident()) {
            writeln!(buf, "    #[serde(default)]")?;
        }

        // Add deserializer attribute for bitcoin::Amount fields
        // Check if the base type (before Option wrapper) is bitcoin::Amount
        if base_field_type == "bitcoin::Amount" {
            // Use different deserializer for Option<Amount> vs Amount
            if field_type.starts_with("Option<") {
                writeln!(buf, "    #[serde(deserialize_with = \"option_amount_from_btc_float\")]")?;
            } else {
                writeln!(buf, "    #[serde(deserialize_with = \"amount_from_btc_float\")]")?;
            }
        }

        writeln!(buf, "    pub {}: {},", field_name, field_type)?;
        Ok(())
    }

    /// Map IR TypeDef to Rust type
    fn map_ir_type_to_rust(&self, type_def: &ir::TypeDef, field_name: &str) -> String {
        let mapped = match &type_def.kind {
            ir::TypeKind::Primitive => {
                // Use the adapter to map the type via BitcoinCoreTypeRegistry.
                // Primitives must have protocol_type ("string", "number", "amount", "hex", etc.).
                let rpc_type =
                    type_def.protocol_type.clone().expect("primitive type must have protocol_type");
                let method_result = types::MethodResult {
                    type_: rpc_type,
                    optional: false,
                    description: type_def.description.clone(),
                    key_name: field_name.to_string(),
                    condition: String::new(),
                    inner: Vec::new(),
                };
                // Use the Bitcoin Core type registry to properly map types
                let (rust_type, _) =
                    adapters::bitcoin_core::types::BitcoinCoreTypeRegistry::map_result_type(
                        &method_result,
                    );
                rust_type.to_string()
            }
            ir::TypeKind::Array => {
                // Array fields must map to Vec<...>, not a single element type.
                // Otherwise serde may deserialize into [T; 32] (e.g. hash) and fail with
                // "invalid length N, expected fewer elements in array" when N > 32.
                let method_result = types::MethodResult {
                    type_: "array".to_string(),
                    optional: false,
                    description: type_def.description.clone(),
                    key_name: field_name.to_string(),
                    condition: String::new(),
                    inner: Vec::new(),
                };
                let (rust_type, _) =
                    adapters::bitcoin_core::types::BitcoinCoreTypeRegistry::map_result_type(
                        &method_result,
                    );
                rust_type.to_string()
            }
            ir::TypeKind::Object => {
                // Decoded tx fields: IR uses Object (with nested array shape) for vin/vout; map to typed vecs.
                match field_name {
                    "vin" => "Vec<DecodedVin>".to_string(),
                    "vout" => "Vec<DecodedVout>".to_string(),
                    _ => "serde_json::Value".to_string(),
                }
            }
            _ => "serde_json::Value".to_string(),
        };
        // If the mapped type is from the bitcoin crate, record it for re-exports
        if let Some(stripped) = mapped.strip_prefix("bitcoin::") {
            let symbol = stripped.split("::").last().unwrap_or(stripped);
            crate::generators::version_specific_response_type::record_external_symbol_usage(
                "bitcoin", symbol,
            );
        }
        mapped
    }

    // Removed generate_fallback_response - no more transparent wrappers

    /// Generate unit response for methods that return "none" or have no meaningful return value
    fn generate_unit_response(&self, method: &RpcDef) -> Result<Option<String>> {
        let struct_name = self.response_struct_name(method);
        let mut buf = String::new();

        // Check if the method has a return type defined in the IR
        if let Some(result) = &method.result {
            match &result.kind {
                ir::TypeKind::Array => {
                    // Generate array wrapper for methods that return arrays
                    return Ok(Some(self.generate_array_wrapper(method, &struct_name, &result)?));
                }
                ir::TypeKind::Primitive => {
                    // Generate primitive wrapper for methods that return primitives
                    return Ok(Some(self.generate_primitive_wrapper(
                        &struct_name,
                        &result.name,
                        &None,
                    )?));
                }
                _ => {
                    // For other types, fall through to unit struct generation
                }
            }
        }

        // Generate struct documentation using PascalCase canonical method name
        let canonical_name =
            crate::utils::canonical_from_adapter_method(&self.implementation, &method.name, None)
                .unwrap_or_else(|_| struct_name.replace("Response", ""));
        write_doc_line(&mut buf, &format!("Response for the `{}` RPC method", canonical_name), "")?;
        writeln!(&mut buf, "///")?;
        write_doc_line(&mut buf, "This method returns no meaningful data.", "")?;

        // Generate unit struct definition
        writeln!(&mut buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(&mut buf, "pub struct {};", struct_name)?;

        Ok(Some(buf))
    }

    /// Check if a method has conditional results (can return different types based on conditions)
    fn check_conditional_results(&self, result: &ir::TypeDef) -> bool {
        // Check if all fields are optional (indicating conditional results)
        if let Some(fields) = &result.fields {
            let fields: Vec<&ir::FieldDef> =
                fields.iter().filter(|f| !Self::is_elision_field(f)).collect();
            if fields.is_empty() {
                return false;
            }
            // If all fields are optional, it's likely a conditional result
            fields.iter().all(|f| !f.required)
        } else {
            false
        }
    }

    /// Generate custom Deserialize implementation for conditional results (string or object)
    fn generate_conditional_deserialize_impl(
        &self,
        buf: &mut String,
        result: &ir::TypeDef,
        struct_name: &str,
    ) -> Result<()> {
        writeln!(buf, "impl<'de> serde::Deserialize<'de> for {} {{", struct_name)?;
        writeln!(buf, "    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>")?;
        writeln!(buf, "    where")?;
        writeln!(buf, "        D: serde::Deserializer<'de>,")?;
        writeln!(buf, "    {{")?;
        writeln!(buf, "        use serde::de::{{self, Visitor}};")?;
        writeln!(buf, "        use std::fmt;")?;
        writeln!(buf)?;
        writeln!(buf, "        struct ConditionalResponseVisitor;")?;
        writeln!(buf)?;
        writeln!(buf, "        #[allow(clippy::needless_lifetimes)]")?;
        writeln!(buf, "        impl<'de> Visitor<'de> for ConditionalResponseVisitor {{")?;
        writeln!(buf, "            type Value = {};", struct_name)?;
        writeln!(buf)?;
        writeln!(
            buf,
            "            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {{"
        )?;
        writeln!(buf, "                formatter.write_str(\"string or object\")")?;
        writeln!(buf, "            }}")?;
        writeln!(buf)?;
        // Handle string case (when verbose=false, e.g. getrawtransaction returns hex string)
        // Find the field that receives the string: "data" (hex) or "txid"
        let fields: Vec<&ir::FieldDef> = result
            .fields
            .as_ref()
            .map(|fs| fs.iter().filter(|f| !Self::is_elision_field(f)).collect())
            .unwrap_or_default();
        let string_field = fields.iter().find(|f| {
            f.key.as_ident() == "data"
                || f.key.as_ident() == "txid"
                || f.key.as_ident().contains("txid")
        });
        let param_name = if string_field.is_some() { "v" } else { "_v" };
        writeln!(
            buf,
            "            fn visit_str<E>(self, {}: &str) -> Result<Self::Value, E>",
            param_name
        )?;
        writeln!(buf, "            where")?;
        writeln!(buf, "                E: de::Error,")?;
        writeln!(buf, "            {{")?;
        if !fields.is_empty() {
            if let Some(field) = string_field {
                let field_name = self.sanitize_identifier(&field.key.as_ident());
                let field_type = self.map_ir_type_to_rust(&field.field_type, &field.key.as_ident());
                // "data" is typically a string (hex); use to_string(); others (e.g. txid) use FromStr
                if field.key.as_ident() == "data" && field_type == "String" {
                    writeln!(
                        buf,
                        "                let {} = {}.to_string();",
                        field_name, param_name
                    )?;
                } else {
                    writeln!(
                        buf,
                        "                let {} = {}::from_str({}).map_err(de::Error::custom)?;",
                        field_name, field_type, param_name
                    )?;
                }
                writeln!(buf, "                Ok({} {{", struct_name)?;
                for f in &fields {
                    let fn_name = self.sanitize_identifier(&f.key.as_ident());
                    if f.key.as_ident() == field.key.as_ident() {
                        writeln!(buf, "                    {}: Some({}),", fn_name, fn_name)?;
                    } else {
                        writeln!(buf, "                    {}: None,", fn_name)?;
                    }
                }
                writeln!(buf, "                }})")?;
            } else {
                // Fallback: create struct with all fields as None
                writeln!(buf, "                Ok({} {{", struct_name)?;
                for f in &fields {
                    let fn_name = self.sanitize_identifier(&f.key.as_ident());
                    writeln!(buf, "                    {}: None,", fn_name)?;
                }
                writeln!(buf, "                }})")?;
            }
        }
        writeln!(buf, "            }}")?;
        writeln!(buf)?;
        // Handle object case (when verbose=true)
        writeln!(
            buf,
            "            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>"
        )?;
        writeln!(buf, "            where")?;
        writeln!(buf, "                M: de::MapAccess<'de>,")?;
        writeln!(buf, "            {{")?;
        if !fields.is_empty() {
            for f in &fields {
                let fn_name = self.sanitize_identifier(&f.key.as_ident());
                writeln!(buf, "                let mut {} = None;", fn_name)?;
            }
            writeln!(buf, "                while let Some(key) = map.next_key::<String>()? {{")?;
            for f in &fields {
                let fn_name = self.sanitize_identifier(&f.key.as_ident());
                writeln!(buf, "                    if key == \"{}\" {{", f.key.as_ident())?;
                writeln!(buf, "                        if {}.is_some() {{", fn_name)?;
                writeln!(
                    buf,
                    "                            return Err(de::Error::duplicate_field(\"{}\"));",
                    f.key.as_ident()
                )?;
                writeln!(buf, "                        }}")?;
                let field_type = self.map_ir_type_to_rust(&f.field_type, &f.key.as_ident());
                let base_field_type = self.map_ir_type_to_rust(&f.field_type, &f.key.as_ident());
                let is_optional = field_type.starts_with("Option<");
                let inner_type = if is_optional {
                    field_type
                        .strip_prefix("Option<")
                        .and_then(|s| s.strip_suffix(">"))
                        .unwrap_or(&base_field_type)
                } else {
                    &base_field_type
                };
                if inner_type == "bitcoin::Amount" {
                    // For Amount types, deserialize as serde_json::Value and convert manually
                    if is_optional {
                        writeln!(buf, "                        let value: Option<serde_json::Value> = map.next_value()?;")?;
                        writeln!(
                            buf,
                            "                        {} = value.and_then(|v| {{",
                            fn_name
                        )?;
                        writeln!(buf, "                            match v {{")?;
                        writeln!(
                            buf,
                            "                                serde_json::Value::Number(n) => {{"
                        )?;
                        writeln!(
                            buf,
                            "                                    if let Some(f) = n.as_f64() {{"
                        )?;
                        writeln!(buf, "                                        bitcoin::Amount::from_btc(f).ok()")?;
                        writeln!(buf, "                                    }} else if let Some(u) = n.as_u64() {{")?;
                        writeln!(buf, "                                        Some(bitcoin::Amount::from_sat(u))")?;
                        writeln!(buf, "                                    }} else if let Some(i) = n.as_i64() {{")?;
                        writeln!(buf, "                                        if i >= 0 {{ Some(bitcoin::Amount::from_sat(i as u64)) }} else {{ None }}")?;
                        writeln!(buf, "                                    }} else {{ None }}")?;
                        writeln!(buf, "                                }}")?;
                        writeln!(buf, "                                _ => None,")?;
                        writeln!(buf, "                            }}")?;
                        writeln!(buf, "                        }});")?;
                    } else {
                        writeln!(buf, "                        let value: serde_json::Value = map.next_value()?;")?;
                        writeln!(buf, "                        {} = Some(match value {{", fn_name)?;
                        writeln!(
                            buf,
                            "                            serde_json::Value::Number(n) => {{"
                        )?;
                        writeln!(
                            buf,
                            "                                if let Some(f) = n.as_f64() {{"
                        )?;
                        writeln!(buf, "                                    bitcoin::Amount::from_btc(f).map_err(|e| de::Error::custom(format!(\"Invalid BTC amount: {{}}\", e)))?")?;
                        writeln!(buf, "                                }} else if let Some(u) = n.as_u64() {{")?;
                        writeln!(
                            buf,
                            "                                    bitcoin::Amount::from_sat(u)"
                        )?;
                        writeln!(buf, "                                }} else if let Some(i) = n.as_i64() {{")?;
                        writeln!(buf, "                                    if i < 0 {{ return Err(de::Error::custom(format!(\"Amount cannot be negative: {{}}\", i))); }}")?;
                        writeln!(buf, "                                    bitcoin::Amount::from_sat(i as u64)")?;
                        writeln!(buf, "                                }} else {{")?;
                        writeln!(buf, "                                    return Err(de::Error::custom(\"Invalid number format for Amount\"));")?;
                        writeln!(buf, "                                }}")?;
                        writeln!(buf, "                            }}")?;
                        writeln!(buf, "                            _ => return Err(de::Error::custom(\"Expected number for Amount field\")),")?;
                        writeln!(buf, "                        }});")?;
                    }
                } else {
                    writeln!(
                        buf,
                        "                        {} = Some(map.next_value::<{}>()?);",
                        fn_name, field_type
                    )?;
                }
                writeln!(buf, "                    }}")?;
            }
            writeln!(buf, "                    else {{")?;
            writeln!(buf, "                        let _ = map.next_value::<de::IgnoredAny>()?;")?;
            writeln!(buf, "                    }}")?;
            writeln!(buf, "                }}")?;
            writeln!(buf, "                Ok({} {{", struct_name)?;
            for f in &fields {
                let fn_name = self.sanitize_identifier(&f.key.as_ident());
                writeln!(buf, "                    {},", fn_name)?;
            }
            writeln!(buf, "                }})")?;
        }
        writeln!(buf, "            }}")?;
        writeln!(buf, "        }}")?;
        writeln!(buf)?;
        writeln!(buf, "        deserializer.deserialize_any(ConditionalResponseVisitor)")?;
        writeln!(buf, "    }}")?;
        writeln!(buf, "}}")?;
        writeln!(buf)?;

        Ok(())
    }

    /// Get response struct name for a method
    fn response_struct_name(&self, method: &RpcDef) -> String {
        let canonical =
            crate::utils::canonical_from_adapter_method(&self.implementation, &method.name, None);
        match canonical {
            Ok(name) => format!("{}Response", name),
            Err(_) => {
                let snake = crate::utils::protocol_rpc_method_to_rust_name(
                    &self.implementation,
                    &method.name,
                )
                .unwrap_or_else(|_| crate::utils::rpc_method_to_rust_name(&method.name));
                format!("{}Response", crate::utils::snake_to_pascal_case(&snake))
            }
        }
    }

    /// Sanitize field name for Rust identifier
    fn sanitize_identifier(&self, name: &str) -> String {
        crate::utils::sanitize_external_identifier(name)
    }

    /// Map metadata type to Rust type
    fn map_metadata_type_to_rust(&self, type_name: &str, is_optional: bool) -> String {
        // Fix incomplete HashMap/BTreeMap types from metadata
        if type_name.contains("HashMap<String") && !type_name.contains(',') {
            // Handle cases like "Option<HashMap<String" or "HashMap<String"
            let fixed = type_name.replace("HashMap<String", "HashMap<String, serde_json::Value>");
            return fixed;
        }

        // Fix specific case: Option<HashMap<String -> Option<HashMap<String, serde_json::Value>>
        if type_name == "Option<HashMap<String" {
            let fixed = "Option<HashMap<String, serde_json::Value>>".to_string();
            return fixed;
        }
        if type_name.contains("BTreeMap<String") && !type_name.contains(',') {
            // Handle cases like "Option<BTreeMap<String" or "BTreeMap<String"
            let fixed = type_name.replace("BTreeMap<String", "BTreeMap<String, serde_json::Value>");
            return fixed;
        }

        // Fix malformed Option wrappers that are missing closing brackets
        if type_name.contains("Option<Option<") && !type_name.ends_with('>') {
            // Count opening and closing brackets to determine how many are missing
            let open_count = type_name.matches('<').count();
            let close_count = type_name.matches('>').count();
            let missing = open_count - close_count;
            let mut fixed = type_name.to_string();
            for _ in 0..missing {
                fixed.push('>');
            }
            return fixed;
        }

        // Fix specific case: Option<Option<HashMap<String, serde_json::Value>>, -> Option<Option<HashMap<String, serde_json::Value>>>
        if type_name.contains("Option<Option<HashMap<String, serde_json::Value>>,") {
            let fixed = type_name.replace(
                "Option<Option<HashMap<String, serde_json::Value>>,",
                "Option<Option<HashMap<String, serde_json::Value>>>",
            );
            return fixed;
        }

        // Fix more general case: any type ending with comma instead of closing bracket
        if type_name.ends_with(',') && type_name.contains('<') {
            let fixed = type_name.trim_end_matches(',').to_string() + ">";
            return fixed;
        }

        // Check if this is already a Rust type (contains < or > or [ or ::)
        if type_name.contains('<')
            || type_name.contains('>')
            || type_name.contains('[')
            || type_name.contains("::")
        {
            // Already a complex Rust type like Vec<T>, [u64; 5], or serde_json::Value, just return it
            return type_name.to_string();
        }

        // Check if this is a custom/structured type (starts with uppercase)
        if type_name.chars().next().is_some_and(|c| c.is_uppercase()) {
            // Handle special case: Transaction conflicts with bitcoin::Transaction
            if type_name == "Transaction" {
                return "BitcoinTransaction".to_string();
            }
            // Custom type, return as-is
            return type_name.to_string();
        }

        // Check if this is already a Rust primitive type
        match type_name {
            "String" | "bool" | "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32"
            | "i64" | "i128" | "f32" | "f64" | "usize" | "isize" | "()" => {
                return type_name.to_string();
            }
            _ => {}
        }

        // Use the adapter to map primitive types
        let method_result = types::MethodResult {
            type_: type_name.to_string(),
            optional: is_optional,
            description: String::new(),
            key_name: String::new(),
            condition: String::new(),
            inner: Vec::new(),
        };

        // Use the Bitcoin Core type registry to properly map types
        let (rust_type, _) =
            adapters::bitcoin_core::types::BitcoinCoreTypeRegistry::map_result_type(&method_result);
        rust_type.to_string()
    }

    /// Helper function to determine if a type is a boolean type
    fn is_bool_type(&self, inner_type: &str) -> bool { inner_type == "bool" }

    /// Helper function to determine if a type is a string type
    fn is_string_type(&self, inner_type: &str) -> bool { inner_type == "String" }

    /// Helper function to determine if a type is a Vec type
    fn is_vec_type(&self, inner_type: &str) -> bool { inner_type.starts_with("Vec<") }

    /// Helper function to determine if a type is a bitcoin::Amount type
    fn is_amount_type(&self, inner_type: &str) -> bool { inner_type == "bitcoin::Amount" }

    /// Helper function to determine if a type is a numeric type
    fn is_numeric_type(&self, inner_type: &str) -> bool {
        matches!(
            inner_type,
            "u8" | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "f32"
                | "f64"
                | "usize"
                | "isize"
        )
    }

    /// Generate a transparent wrapper for primitive return types
    fn generate_primitive_wrapper(
        &self,
        struct_name: &str,
        type_name: &str,
        doc_comment: &Option<String>,
    ) -> Result<String> {
        let mut buf = String::new();

        // Generate struct documentation using PascalCase canonical method name
        if let Some(doc) = doc_comment {
            write_doc_line(&mut buf, doc, "")?;
        } else {
            // Use PascalCase from struct name
            let canonical_name = struct_name.replace("Response", "");
            write_doc_line(
                &mut buf,
                &format!("Response for the `{}` RPC method", canonical_name),
                "",
            )?;
        }
        writeln!(&mut buf, "///")?;
        write_doc_line(
            &mut buf,
            "This method returns a primitive value wrapped in a transparent struct.",
            "",
        )?;

        // Map the type name to Rust type
        let inner_type = self.map_metadata_type_to_rust(type_name, false);

        let is_unit = inner_type.trim() == "()";

        // Generate transparent wrapper struct with custom deserializer
        writeln!(&mut buf, "#[derive(Debug, Clone, PartialEq, Serialize)]")?;
        writeln!(&mut buf, "pub struct {} {{", struct_name)?;
        write_doc_line(&mut buf, "Wrapped primitive value", "    ")?;
        writeln!(&mut buf, "    pub value: {},", inner_type)?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        // Generate custom Deserialize implementation
        writeln!(&mut buf, "impl<'de> serde::Deserialize<'de> for {} {{", struct_name)?;
        writeln!(&mut buf, "    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>")?;
        writeln!(&mut buf, "    where")?;
        writeln!(&mut buf, "        D: serde::Deserializer<'de>,")?;
        writeln!(&mut buf, "    {{")?;
        writeln!(&mut buf, "        use serde::de::{{self, Visitor}};")?;
        writeln!(&mut buf, "        use std::fmt;")?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "        struct PrimitiveWrapperVisitor;")?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "        #[allow(unused_variables, clippy::needless_lifetimes)]")?;
        writeln!(&mut buf, "        impl<'de> Visitor<'de> for PrimitiveWrapperVisitor {{")?;
        writeln!(&mut buf, "            type Value = {};", struct_name)?;
        writeln!(&mut buf)?;
        writeln!(
            &mut buf,
            "            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {{"
        )?;
        writeln!(&mut buf, "                formatter.write_str(\"a primitive value or an object with 'value' field\")")?;
        writeln!(&mut buf, "            }}")?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>")?;
        writeln!(&mut buf, "            where")?;
        writeln!(&mut buf, "                E: de::Error,")?;
        writeln!(&mut buf, "            {{")?;
        if is_unit {
            writeln!(&mut buf, "                Ok({} {{ value: () }})", struct_name)?;
        } else if self.is_bool_type(&inner_type) {
            writeln!(&mut buf, "                Ok({} {{ value: v != 0 }})", struct_name)?;
        } else if self.is_string_type(&inner_type) {
            writeln!(&mut buf, "                Ok({} {{ value: v.to_string() }})", struct_name)?;
        } else if self.is_vec_type(&inner_type) {
            writeln!(
                &mut buf,
                "                Err(de::Error::custom(\"cannot convert u64 to Vec type\"))"
            )?;
        } else if self.is_amount_type(&inner_type) {
            writeln!(
                &mut buf,
                "                Ok({} {{ value: bitcoin::Amount::from_sat(v) }})",
                struct_name
            )?;
        } else if self.is_numeric_type(&inner_type) {
            if inner_type == "u64" {
                writeln!(&mut buf, "                Ok({} {{ value: v }})", struct_name)?;
            } else {
                writeln!(
                    &mut buf,
                    "                Ok({} {{ value: v as {} }})",
                    struct_name, inner_type
                )?;
            }
        } else {
            writeln!(
                &mut buf,
                "                Err(de::Error::custom(\"cannot convert u64 to {}\"))",
                inner_type
            )?;
        }
        writeln!(&mut buf, "            }}")?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>")?;
        writeln!(&mut buf, "            where")?;
        writeln!(&mut buf, "                E: de::Error,")?;
        writeln!(&mut buf, "            {{")?;
        if is_unit {
            writeln!(&mut buf, "                Ok({} {{ value: () }})", struct_name)?;
        } else if self.is_bool_type(&inner_type) {
            writeln!(&mut buf, "                Ok({} {{ value: v != 0 }})", struct_name)?;
        } else if self.is_string_type(&inner_type) {
            writeln!(&mut buf, "                Ok({} {{ value: v.to_string() }})", struct_name)?;
        } else if self.is_vec_type(&inner_type) {
            writeln!(
                &mut buf,
                "                Err(de::Error::custom(\"cannot convert i64 to Vec type\"))"
            )?;
        } else if self.is_amount_type(&inner_type) {
            writeln!(&mut buf, "                if v < 0 {{")?;
            writeln!(
                &mut buf,
                "                    return Err(de::Error::custom(format!(\"Amount cannot be negative: {{}}\", v)));"
            )?;
            writeln!(&mut buf, "                }}")?;
            writeln!(
                &mut buf,
                "                Ok({} {{ value: bitcoin::Amount::from_sat(v as u64) }})",
                struct_name
            )?;
        } else if self.is_numeric_type(&inner_type) {
            if inner_type == "u64" {
                writeln!(&mut buf, "                Ok({} {{ value: v as u64 }})", struct_name)?;
            } else {
                writeln!(
                    &mut buf,
                    "                Ok({} {{ value: v as {} }})",
                    struct_name, inner_type
                )?;
            }
        } else {
            writeln!(
                &mut buf,
                "                Err(de::Error::custom(\"cannot convert i64 to {}\"))",
                inner_type
            )?;
        }
        writeln!(&mut buf, "            }}")?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>")?;
        writeln!(&mut buf, "            where")?;
        writeln!(&mut buf, "                E: de::Error,")?;
        writeln!(&mut buf, "            {{")?;
        if is_unit {
            writeln!(&mut buf, "                Ok({} {{ value: () }})", struct_name)?;
        } else if self.is_bool_type(&inner_type) {
            writeln!(&mut buf, "                Ok({} {{ value: v != 0.0 }})", struct_name)?;
        } else if self.is_string_type(&inner_type) {
            writeln!(&mut buf, "                Ok({} {{ value: v.to_string() }})", struct_name)?;
        } else if self.is_vec_type(&inner_type) {
            writeln!(
                &mut buf,
                "                Err(de::Error::custom(\"cannot convert f64 to Vec type\"))"
            )?;
        } else if self.is_amount_type(&inner_type) {
            writeln!(
                &mut buf,
                "                let amount = bitcoin::Amount::from_btc(v).map_err(|e| de::Error::custom(format!(\"Invalid BTC amount: {{}}\", e)))?;"
            )?;
            writeln!(&mut buf, "                Ok({} {{ value: amount }})", struct_name)?;
        } else if self.is_numeric_type(&inner_type) {
            if inner_type == "u64" {
                writeln!(&mut buf, "                Ok({} {{ value: v as u64 }})", struct_name)?;
            } else {
                writeln!(
                    &mut buf,
                    "                Ok({} {{ value: v as {} }})",
                    struct_name, inner_type
                )?;
            }
        } else {
            writeln!(
                &mut buf,
                "                Err(de::Error::custom(\"cannot convert f64 to {}\"))",
                inner_type
            )?;
        }
        writeln!(&mut buf, "            }}")?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>")?;
        writeln!(&mut buf, "            where")?;
        writeln!(&mut buf, "                E: de::Error,")?;
        writeln!(&mut buf, "            {{")?;
        if is_unit {
            writeln!(&mut buf, "                Ok({} {{ value: () }})", struct_name)?;
        } else if self.is_bool_type(&inner_type) {
            writeln!(
                &mut buf,
                "                let value = v.parse::<bool>().map_err(de::Error::custom)?;"
            )?;
            writeln!(&mut buf, "                Ok({} {{ value }})", struct_name)?;
        } else if self.is_string_type(&inner_type) {
            writeln!(&mut buf, "                Ok({} {{ value: v.to_string() }})", struct_name)?;
        } else if self.is_vec_type(&inner_type) {
            writeln!(
                &mut buf,
                "                Err(de::Error::custom(\"cannot convert string to Vec type\"))"
            )?;
        } else {
            writeln!(
                &mut buf,
                "                let value = v.parse::<{}>().map_err(de::Error::custom)?;",
                inner_type
            )?;
            writeln!(&mut buf, "                Ok({} {{ value }})", struct_name)?;
        }
        writeln!(&mut buf, "            }}")?;
        writeln!(&mut buf)?;
        writeln!(
            &mut buf,
            "            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>"
        )?;
        writeln!(&mut buf, "            where")?;
        writeln!(&mut buf, "                E: de::Error,")?;
        writeln!(&mut buf, "            {{")?;
        if is_unit {
            writeln!(&mut buf, "                Ok({} {{ value: () }})", struct_name)?;
        } else if self.is_bool_type(&inner_type) {
            writeln!(&mut buf, "                Ok({} {{ value: v }})", struct_name)?;
        } else if self.is_string_type(&inner_type) {
            writeln!(&mut buf, "                Ok({} {{ value: v.to_string() }})", struct_name)?;
        } else if self.is_vec_type(&inner_type) {
            writeln!(
                &mut buf,
                "                Err(de::Error::custom(\"cannot convert bool to Vec type\"))"
            )?;
        } else if inner_type == "f64" {
            writeln!(
                &mut buf,
                "                Ok({} {{ value: if v {{ 1.0 }} else {{ 0.0 }} }})",
                struct_name
            )?;
        } else if self.is_numeric_type(&inner_type) {
            writeln!(
                &mut buf,
                "                Ok({} {{ value: v as {} }})",
                struct_name, inner_type
            )?;
        } else {
            writeln!(
                &mut buf,
                "                Err(de::Error::custom(\"cannot convert bool to {}\"))",
                inner_type
            )?;
        }
        writeln!(&mut buf, "            }}")?;
        writeln!(&mut buf)?;
        // Handle null JSON values (for unit types like () that return null)
        if is_unit {
            writeln!(&mut buf, "            fn visit_none<E>(self) -> Result<Self::Value, E>")?;
            writeln!(&mut buf, "            where")?;
            writeln!(&mut buf, "                E: de::Error,")?;
            writeln!(&mut buf, "            {{")?;
            writeln!(&mut buf, "                Ok({} {{ value: () }})", struct_name)?;
            writeln!(&mut buf, "            }}")?;
            writeln!(&mut buf)?;
            writeln!(&mut buf, "            fn visit_unit<E>(self) -> Result<Self::Value, E>")?;
            writeln!(&mut buf, "            where")?;
            writeln!(&mut buf, "                E: de::Error,")?;
            writeln!(&mut buf, "            {{")?;
            writeln!(&mut buf, "                Ok({} {{ value: () }})", struct_name)?;
            writeln!(&mut buf, "            }}")?;
            writeln!(&mut buf)?;
        }
        if self.is_vec_type(&inner_type) {
            writeln!(
                &mut buf,
                "            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>"
            )?;
            writeln!(&mut buf, "            where")?;
            writeln!(&mut buf, "                A: de::SeqAccess<'de>,")?;
            writeln!(&mut buf, "            {{")?;
            writeln!(&mut buf, "                let mut values = Vec::new();")?;
            writeln!(&mut buf, "                while let Some(value) = seq.next_element()? {{")?;
            writeln!(&mut buf, "                    values.push(value);")?;
            writeln!(&mut buf, "                }}")?;
            writeln!(&mut buf, "                Ok({} {{ value: values }})", struct_name)?;
            writeln!(&mut buf, "            }}")?;
            writeln!(&mut buf)?;
        }
        writeln!(
            &mut buf,
            "            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>"
        )?;
        writeln!(&mut buf, "            where")?;
        writeln!(&mut buf, "                M: de::MapAccess<'de>,")?;
        writeln!(&mut buf, "            {{")?;
        writeln!(&mut buf, "                let mut value = None;")?;
        writeln!(&mut buf, "                while let Some(key) = map.next_key::<String>()? {{")?;
        writeln!(&mut buf, "                    if key == \"value\" {{")?;
        writeln!(&mut buf, "                        if value.is_some() {{")?;
        writeln!(
            &mut buf,
            "                            return Err(de::Error::duplicate_field(\"value\"));"
        )?;
        writeln!(&mut buf, "                        }}")?;
        if is_unit {
            writeln!(&mut buf, "                        value = Some(map.next_value::<()>()?);")?;
        } else {
            writeln!(&mut buf, "                        value = Some(map.next_value()?);")?;
        }
        writeln!(&mut buf, "                    }} else {{")?;
        writeln!(&mut buf, "                        let _ = map.next_value::<de::IgnoredAny>()?;")?;
        writeln!(&mut buf, "                    }}")?;
        writeln!(&mut buf, "                }}")?;
        if inner_type.trim() == "()" {
            writeln!(
                &mut buf,
                "                value.ok_or_else(|| de::Error::missing_field(\"value\"))?;"
            )?;
            writeln!(&mut buf, "                Ok({} {{ value: () }})", struct_name)?;
        } else {
            writeln!(&mut buf, "                let value = value.ok_or_else(|| de::Error::missing_field(\"value\"))?;")?;
            writeln!(&mut buf, "                Ok({} {{ value }})", struct_name)?;
        }
        writeln!(&mut buf, "            }}")?;
        writeln!(&mut buf, "        }}")?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "        deserializer.deserialize_any(PrimitiveWrapperVisitor)")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        // Generate Deref implementation for ergonomic access
        writeln!(&mut buf, "impl std::ops::Deref for {} {{", struct_name)?;
        writeln!(&mut buf, "    type Target = {};", inner_type)?;
        writeln!(&mut buf, "    fn deref(&self) -> &Self::Target {{")?;
        writeln!(&mut buf, "        &self.value")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        // Generate DerefMut implementation for mutable access
        writeln!(&mut buf, "impl std::ops::DerefMut for {} {{", struct_name)?;
        writeln!(&mut buf, "    fn deref_mut(&mut self) -> &mut Self::Target {{")?;
        writeln!(&mut buf, "        &mut self.value")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        // Generate AsRef implementation
        writeln!(&mut buf, "impl AsRef<{}> for {} {{", inner_type, struct_name)?;
        writeln!(&mut buf, "    fn as_ref(&self) -> &{} {{", inner_type)?;
        writeln!(&mut buf, "        &self.value")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        // Generate From implementation for transparent conversion
        writeln!(&mut buf, "impl From<{}> for {} {{", inner_type, struct_name)?;
        writeln!(&mut buf, "    fn from(value: {}) -> Self {{", inner_type)?;
        writeln!(&mut buf, "        Self {{ value }}")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        // Generate Into implementation for transparent conversion
        writeln!(&mut buf, "impl From<{}> for {} {{", struct_name, inner_type)?;
        writeln!(&mut buf, "    fn from(wrapper: {}) -> Self {{", struct_name)?;
        writeln!(&mut buf, "        wrapper.value")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;

        Ok(buf)
    }

    /// Generate array wrapper for methods that return arrays.
    /// When IR encodes the element type (TypeKind::Array with a single anonymous field),
    /// we derive the Rust element type from that; otherwise we fall back to `serde_json::Value`.
    fn generate_array_wrapper(
        &self,
        method: &RpcDef,
        struct_name: &str,
        result: &ir::TypeDef,
    ) -> Result<String> {
        let mut buf = String::new();
        let value_ty = if let Some(elem_ty) = Self::array_element_type_from_ir(result) {
            self.map_ir_type_to_rust(elem_ty, "field_0")
        } else {
            "serde_json::Value".to_string()
        };

        if let Some(stripped) = value_ty.strip_prefix("bitcoin::") {
            let symbol = stripped.split("::").last().unwrap_or(stripped);
            record_external_symbol("bitcoin", symbol);
        }

        let vec_ty = format!("Vec<{}>", value_ty);

        // Generate struct documentation using PascalCase canonical method name
        let canonical_name =
            crate::utils::canonical_from_adapter_method(&self.implementation, &method.name, None)
                .unwrap_or_else(|_| struct_name.replace("Response", "").to_string());
        write_doc_line(&mut buf, &format!("Response for the `{}` RPC method", canonical_name), "")?;
        writeln!(&mut buf, "///")?;
        write_doc_line(
            &mut buf,
            "This method returns an array wrapped in a transparent struct.",
            "",
        )?;

        // Generate transparent wrapper struct
        writeln!(&mut buf, "#[derive(Debug, Clone, PartialEq, Serialize)]")?;
        writeln!(&mut buf, "pub struct {} {{", struct_name)?;
        write_doc_line(&mut buf, "Wrapped array value", "    ")?;
        writeln!(&mut buf, "    pub value: {},", vec_ty)?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        // Generate custom Deserialize implementation
        writeln!(&mut buf, "impl<'de> serde::Deserialize<'de> for {} {{", struct_name)?;
        writeln!(&mut buf, "    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>")?;
        writeln!(&mut buf, "    where")?;
        writeln!(&mut buf, "        D: serde::Deserializer<'de>,")?;
        writeln!(&mut buf, "    {{")?;
        writeln!(&mut buf, "        let value = Vec::<{}>::deserialize(deserializer)?;", value_ty)?;
        writeln!(&mut buf, "        Ok(Self {{ value }})")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        // Generate From implementations for transparent conversion
        writeln!(&mut buf, "impl From<{}> for {} {{", vec_ty, struct_name)?;
        writeln!(&mut buf, "    fn from(value: {}) -> Self {{", vec_ty)?;
        writeln!(&mut buf, "        Self {{ value }}")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        writeln!(&mut buf, "impl From<{}> for {} {{", struct_name, vec_ty)?;
        writeln!(&mut buf, "    fn from(wrapper: {}) -> Self {{", struct_name)?;
        writeln!(&mut buf, "        wrapper.value")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;

        Ok(buf)
    }

    /// Generate wrapper for RPCs that return arbitrary JSON (echo/echojson); whole response is one Value
    fn generate_value_wrapper(&self, method: &RpcDef, struct_name: &str) -> Result<String> {
        let mut buf = String::new();
        let canonical_name =
            crate::utils::canonical_from_adapter_method(&self.implementation, &method.name, None)
                .unwrap_or_else(|_| struct_name.replace("Response", "").to_string());
        write_doc_line(&mut buf, &format!("Response for the `{}` RPC method", canonical_name), "")?;
        writeln!(&mut buf, "///")?;
        write_doc_line(
            &mut buf,
            "This method returns arbitrary JSON (e.g. string, object, array) as a single value.",
            "",
        )?;

        writeln!(&mut buf, "#[derive(Debug, Clone, PartialEq, Serialize)]")?;
        writeln!(&mut buf, "pub struct {} {{", struct_name)?;
        write_doc_line(&mut buf, "Wrapped JSON value", "    ")?;
        writeln!(&mut buf, "    pub value: serde_json::Value,")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        writeln!(&mut buf, "impl<'de> serde::Deserialize<'de> for {} {{", struct_name)?;
        writeln!(&mut buf, "    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>")?;
        writeln!(&mut buf, "    where")?;
        writeln!(&mut buf, "        D: serde::Deserializer<'de>,")?;
        writeln!(&mut buf, "    {{")?;
        writeln!(&mut buf, "        let value = serde_json::Value::deserialize(deserializer)?;")?;
        writeln!(&mut buf, "        Ok(Self {{ value }})")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        writeln!(&mut buf, "impl From<serde_json::Value> for {} {{", struct_name)?;
        writeln!(&mut buf, "    fn from(value: serde_json::Value) -> Self {{")?;
        writeln!(&mut buf, "        Self {{ value }}")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;

        Ok(buf)
    }

    /// Recursively collect nested type names from a TypeDef (so nested scriptPubKey is seen).
    fn collect_nested_types_from_type_def(
        &self,
        type_def: &ir::TypeDef,
        nested_types: &mut BTreeSet<String>,
    ) {
        self.collect_nested_types(&type_def.name, nested_types);
        if let Some(ref fields) = type_def.fields {
            for field in fields {
                self.collect_nested_types_from_type_def(&field.field_type, nested_types);
            }
        }
    }

    /// Collect nested types from a field type string
    fn collect_nested_types(&self, type_name: &str, nested_types: &mut BTreeSet<String>) {
        // Extract type names that start with uppercase (custom types)
        let words: Vec<&str> = type_name.split(|c: char| !c.is_alphanumeric()).collect();
        for word in words {
            if !word.is_empty() && word.chars().next().is_some_and(|c| c.is_uppercase()) {
                // Skip known primitive types and common types
                match word {
                    "String" | "Option" | "Vec" | "HashMap" | "BTreeMap" | "serde_json"
                    | "Value" => {
                        continue;
                    }
                    _ => {
                        // Collect nested types - simplified since IR doesn't track per-type versions
                        nested_types.insert(word.to_string());
                    }
                }
            }
        }
    }

    /// Names of response types that we emit as full structs (not from IR).
    /// Includes decoded-tx helper structs and GetBlockTemplateTransaction.
    const MANUAL_RESPONSE_TYPE_NAMES: &[&str] = &[
        "DecodedScriptSig",
        "DecodedPrevout",
        "DecodedVin",
        "DecodedVout",
        "DecodedTxDetails",
        "GetBlockTemplateTransaction",
    ];

    /// Generate a nested type: from type registry when present, else skip (decoded-tx) or type alias.
    fn generate_nested_type(
        &self,
        type_name: &str,
        type_registry: &BTreeMap<String, TypeDef>,
    ) -> Result<Option<String>> {
        if let Some(type_def) = type_registry.get(type_name) {
            if matches!(type_def.kind, TypeKind::Object) && type_def.fields.is_some() {
                let mut buf = String::new();
                self.generate_struct_from_type_def(type_def, &mut buf)?;
                return Ok(Some(buf));
            }
        }
        // Skip types that we emit as full structs via manual helpers (not from IR).
        if Self::MANUAL_RESPONSE_TYPE_NAMES.contains(&type_name) {
            return Ok(None);
        }
        let type_alias = self.generate_type_alias(type_name)?;
        Ok(Some(type_alias))
    }

    /// Generate a type alias for types not found in metadata
    fn generate_type_alias(&self, type_name: &str) -> Result<String> {
        // Check if this type is already imported from external crates
        if self.is_external_type(type_name) {
            return Ok(String::new());
        }

        // Map common Bitcoin Core types to their appropriate Rust types
        let rust_type = match type_name {
            "Bip125Replaceable" => "bool", // Boolean for replaceable status
            "ScriptPubkey" => "bitcoin::ScriptBuf", // Hex-encoded script pubkey
            _ => "String",                 // Default to String
        };

        let mut output = String::new();
        write_doc_line(&mut output, &format!("Type alias for {}", type_name), "")?;
        writeln!(output, "pub type {} = {};", type_name, rust_type)?;
        Ok(output)
    }

    /// Check if a type is imported from external crates
    fn is_external_type(&self, type_name: &str) -> bool {
        matches!(
            type_name,
            "ScriptBuf" | "Transaction" | "TxOut" | "KeySource" | "TapTree" | "ProprietaryKey"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_wrapper_uses_ir_element_type() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        // IR: top-level array of primitive strings.
        let elem_ty = TypeDef {
            name: "string".to_string(),
            description: String::new(),
            kind: TypeKind::Primitive,
            fields: None,
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("string".to_string()),
            canonical_name: None,
            condition: None,
        };

        // Element encoded as anonymous positional field_0.
        let result_ty = TypeDef {
            name: "array".to_string(),
            description: String::new(),
            kind: TypeKind::Array,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Anonymous(0),
                field_type: elem_ty,
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("array".to_string()),
            canonical_name: None,
            condition: None,
        };

        let method = RpcDef {
            name: "deriveaddresses".to_string(),
            description: String::new(),
            params: Vec::new(),
            result: Some(result_ty),
            category: String::new(),
            access_level: ir::AccessLevel::Public,
            requires_private_keys: false,
            version_added: None,
            version_removed: None,
            examples: None,
            hidden: None,
        };

        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");

        // We expect a transparent wrapper over Vec<String>.
        assert!(
            code.contains("pub value: Vec<String>"),
            "expected Vec<String> field, got:\n{code}"
        );
    }

    #[test]
    fn array_wrapper_recognizes_named_field_0_element() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        // IR: top-level array of primitive strings with a synthetic Named(\"field_0\") key.
        let elem_ty = TypeDef {
            name: "string".to_string(),
            description: String::new(),
            kind: TypeKind::Primitive,
            fields: None,
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("string".to_string()),
            canonical_name: None,
            condition: None,
        };

        let result_ty = TypeDef {
            name: "array".to_string(),
            description: String::new(),
            kind: TypeKind::Array,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Named("field_0".to_string()),
                field_type: elem_ty,
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("array".to_string()),
            canonical_name: None,
            condition: None,
        };

        let method = RpcDef {
            name: "deriveaddresses".to_string(),
            description: String::new(),
            params: Vec::new(),
            result: Some(result_ty),
            category: String::new(),
            access_level: ir::AccessLevel::Public,
            requires_private_keys: false,
            version_added: None,
            version_removed: None,
            examples: None,
            hidden: None,
        };

        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");

        // We still expect a transparent wrapper over Vec<String>.
        assert!(
            code.contains("pub value: Vec<String>"),
            "expected Vec<String> field for Named(\"field_0\") element, got:\n{code}"
        );
    }

    #[test]
    fn array_wrapper_uses_value_vec_for_any() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        // IR: top-level array of primitive `any` (maps to serde_json::Value).
        let elem_ty = TypeDef {
            name: "any".to_string(),
            description: String::new(),
            kind: TypeKind::Primitive,
            fields: None,
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("any".to_string()),
            canonical_name: None,
            condition: None,
        };

        let result_ty = TypeDef {
            name: "array".to_string(),
            description: String::new(),
            kind: TypeKind::Array,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Anonymous(0),
                field_type: elem_ty,
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("array".to_string()),
            canonical_name: None,
            condition: None,
        };

        let method = RpcDef {
            name: "getrawmempool".to_string(),
            description: String::new(),
            params: Vec::new(),
            result: Some(result_ty),
            category: String::new(),
            access_level: ir::AccessLevel::Public,
            requires_private_keys: false,
            version_added: None,
            version_removed: None,
            examples: None,
            hidden: None,
        };

        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");

        // We expect a transparent wrapper over Vec<serde_json::Value>.
        assert!(
            code.contains("pub value: Vec<serde_json::Value>"),
            "expected Vec<serde_json::Value> field, got:\n{code}"
        );
    }

    #[test]
    fn getblocktemplate_placeholder_field_optional() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        // IR: object result whose first anonymous field encodes the proposal-accepted `none` result.
        let none_ty = TypeDef {
            name: "none".to_string(),
            description: String::new(),
            kind: TypeKind::Primitive,
            fields: None,
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("none".to_string()),
            canonical_name: None,
            condition: Some("If the proposal was accepted with mode=='proposal'".to_string()),
        };

        let version_ty = TypeDef {
            name: "number".to_string(),
            description: String::new(),
            kind: TypeKind::Primitive,
            fields: None,
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("number".to_string()),
            canonical_name: None,
            condition: None,
        };

        let result_ty = TypeDef {
            name: "object".to_string(),
            description: String::new(),
            kind: TypeKind::Object,
            fields: Some(vec![
                ir::FieldDef {
                    key: ir::FieldKey::Anonymous(0),
                    field_type: none_ty,
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                },
                ir::FieldDef {
                    key: ir::FieldKey::Named("version".to_string()),
                    field_type: version_ty,
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                },
            ]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("object".to_string()),
            canonical_name: None,
            condition: None,
        };

        let method = RpcDef {
            name: "getblocktemplate".to_string(),
            description: String::new(),
            params: Vec::new(),
            result: Some(result_ty),
            category: String::new(),
            access_level: ir::AccessLevel::Public,
            requires_private_keys: false,
            version_added: None,
            version_removed: None,
            examples: None,
            hidden: None,
        };

        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");

        // The placeholder field should be optional with a serde default attribute.
        assert!(
            code.contains("#[serde(default)]"),
            "expected serde default attr for placeholder field, got:\n{code}"
        );
        assert!(
            code.contains("pub field_0: Option<()>"),
            "expected optional unit placeholder field, got:\n{code}"
        );
    }

    /// Regression test for the BTreeMap/BTreeSet stabilization: when the set of nested types
    /// changes (e.g. one method removed), HashSet iteration order can change, so the remaining
    /// structs appear in a different order and the diff is noisy. With BTreeSet, order is
    /// deterministic (sorted), so the relative order of remaining structs is stable.
    ///
    /// Type names Aa, Bb, Cc are chosen so that with HashSet the iteration order differs
    /// when the set shrinks from 3 to 2 elements; this test then fails. With BTreeSet it passes.
    #[test]
    fn nested_type_emission_order_stable_when_set_shrinks() {
        use std::str::FromStr;

        use ir::AccessLevel;

        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        fn object_type(name: &str) -> TypeDef {
            let string_ty = TypeDef {
                name: "string".to_string(),
                description: String::new(),
                kind: TypeKind::Primitive,
                fields: None,
                variants: None,
                union_variants: None,
                base_type: None,
                protocol_type: Some("string".to_string()),
                canonical_name: None,
                condition: None,
            };
            TypeDef {
                name: name.to_string(),
                description: String::new(),
                kind: TypeKind::Object,
                fields: Some(vec![ir::FieldDef {
                    key: ir::FieldKey::Named("x".to_string()),
                    field_type: string_ty,
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                }]),
                variants: None,
                union_variants: None,
                base_type: None,
                protocol_type: None,
                canonical_name: None,
                condition: None,
            }
        }

        fn rpc_method(name: &str, result: TypeDef) -> RpcDef {
            RpcDef {
                name: name.to_string(),
                description: String::new(),
                params: Vec::new(),
                result: Some(result),
                category: String::new(),
                access_level: AccessLevel::Public,
                requires_private_keys: false,
                version_added: None,
                version_removed: None,
                examples: None,
                hidden: None,
            }
        }

        // Use short names with spread hash values so HashSet iteration order is more likely
        // to differ when set size changes (3 → 2), reproducing the bug with hash-based collections.
        let type_a = object_type("Aa");
        let type_b = object_type("Bb");
        let type_c = object_type("Cc");

        let method_a = rpc_method("method_a", type_a.clone());
        let method_b = rpc_method("method_b", type_b.clone());
        let method_c = rpc_method("method_c", type_c);

        // Run 1: all three methods → nested set {Aa, Bb, Cc}
        let out1 = gen
            .generate(&[method_a.clone(), method_b.clone(), method_c.clone()])
            .expect("generate must succeed");
        let content1 = &out1[0].1;

        // Run 2: remove method_c → nested set {Aa, Bb} (set shrank)
        let out2 = gen.generate(&[method_a, method_b]).expect("generate must succeed");
        let content2 = &out2[0].1;

        fn order_of(content: &str, names: &[&str]) -> Vec<usize> {
            names
                .iter()
                .map(|n| {
                    content
                        .find(&format!("pub struct {}", n))
                        .unwrap_or_else(|| panic!("struct {} not found in output", n))
                })
                .collect()
        }

        let names = ["Aa", "Bb"];
        let positions1 = order_of(content1, &names);
        let positions2 = order_of(content2, &names);

        // With BTreeSet: order is deterministic (sorted). So NestedTypeA < NestedTypeB in both runs.
        // With HashSet: when set shrinks from 3 to 2, iteration order can change, so NestedTypeA and
        // NestedTypeB might swap → relative order in run2 would differ from run1 → assertion fails.
        let order_a_before_b_run1 = positions1[0] < positions1[1];
        let order_a_before_b_run2 = positions2[0] < positions2[1];
        assert!(
            order_a_before_b_run1 == order_a_before_b_run2,
            "nested type emission order must be stable when the set shrinks (use BTreeSet, not HashSet). \
             Run1 order: {:?}, run2 order: {:?}",
            positions1,
            positions2
        );
    }
}
