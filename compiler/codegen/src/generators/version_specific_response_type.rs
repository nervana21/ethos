//! Version-specific response type generator
//!
//! This module enhances the response type generator to use version-specific
//! type metadata extracted from corepc to generate accurate types for each
//! Bitcoin Core version.
//!
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};

use ir::{ProtocolIR, RpcDef, TypeDef, TypeKind, UnionVariantDef};
use serde::Serialize;
use types::{Implementation, ProtocolVersion};

use super::doc_comment::{write_doc_comment, write_doc_line};
use crate::utils::sanitize_type_name_for_rust;
use crate::Result;

// Type alias to reduce type complexity
type SymbolRecorder = fn(&str, &str);

// Safe global to record external symbol usage via a callback
static EXTERNAL_SYMBOL_RECORDER: OnceLock<Mutex<Option<SymbolRecorder>>> = OnceLock::new();
static FALLBACK_EVENTS: OnceLock<Mutex<Vec<FallbackEvent>>> = OnceLock::new();
static CURRENT_RPC_METHOD: OnceLock<Mutex<Option<String>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
/// One weak-typing event emitted while mapping IR to Rust response types.
pub struct FallbackEvent {
    /// RPC method name when known.
    pub rpc_method: String,
    /// Best-effort pointer to schema/IR location.
    pub schema_or_ir_path: String,
    /// Category of fallback behavior.
    pub fallback_kind: String,
    /// Rust type selected by the fallback.
    pub chosen_rust_type: String,
    /// Machine-readable reason for the fallback.
    pub reason: String,
    /// Severity bucket (`P0`/`P1`/`P2`).
    pub severity: String,
}

/// Clears all collected fallback events.
pub fn clear_fallback_events() {
    let slot = FALLBACK_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
    let mut guard = slot.lock().expect("fallback event mutex poisoned");
    guard.clear();
}

/// Returns a snapshot of currently collected fallback events.
pub fn fallback_events_snapshot() -> Vec<FallbackEvent> {
    let slot = FALLBACK_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
    let guard = slot.lock().expect("fallback event mutex poisoned");
    guard.clone()
}

fn record_fallback_event(event: FallbackEvent) {
    let slot = FALLBACK_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
    let mut guard = slot.lock().expect("fallback event mutex poisoned");
    guard.push(event);
}

fn set_current_rpc_method(name: Option<&str>) {
    let slot = CURRENT_RPC_METHOD.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().expect("current rpc mutex poisoned");
    *guard = name.map(ToString::to_string);
}

fn current_rpc_method_or_unknown() -> String {
    let slot = CURRENT_RPC_METHOD.get_or_init(|| Mutex::new(None));
    let guard = slot.lock().expect("current rpc mutex poisoned");
    guard.clone().unwrap_or_else(|| "unknown".to_string())
}

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
    #[inline]
    fn ir_rust_type_label(ty: &ir::TypeDef) -> String {
        sanitize_type_name_for_rust(ty.rust_emit_name())
    }

    /// Prefix merged branch struct names with their parent `#[serde(untagged)]` enum so IR
    /// collisions like `FinalizePsbtBranch1Object1` under both root and nested unions become
    /// distinct Rust types.
    #[inline]
    fn qualify_union_branch_struct(parent_enum: &str, inner_label: &str) -> String {
        sanitize_type_name_for_rust(&format!("{parent_enum}{inner_label}"))
    }

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
        let mut out = String::from("// Generated version-specific RPC response types\n");
        out.push_str("//\n");
        let implementation_display = Implementation::from_str(&self.implementation)
            .map(|impl_| impl_.display_name().to_string())
            .unwrap_or_else(|_| self.implementation.clone());
        out.push_str(&format!(
            "// Generated for {} {}\n",
            implementation_display,
            self.version.short()
        ));
        out.push_str("//\n");
        out.push_str("// These types are version-specific and may not match other versions.\n");

        // First, collect all nested types that need to be generated
        // BTreeSet for deterministic iteration order so generated output is stable.
        let mut nested_types = BTreeSet::new();
        for method in methods {
            if let Some(result) = &method.result {
                self.collect_nested_types_from_type_def(result, &mut nested_types, true);
            }
        }
        // Also collect nested types from the actual Rust type strings we will emit for fields.
        // Some types are introduced by adapter type-mapping (BitcoinCoreTypeRegistry) and therefore
        // do not appear as IR TypeDef names.
        for method in methods {
            if let Some(result) = &method.result {
                let result = self.filter_type_def_for_version(result);
                let mut scan = |fields: &[ir::FieldDef]| {
                    for field in fields {
                        let rust_type = self.map_ir_type_to_rust(
                            &field.field_type,
                            &field.key.as_ident(),
                            None,
                        );
                        self.collect_nested_types(&rust_type, &mut nested_types);
                    }
                };
                if let Some(fields) = &result.fields {
                    scan(fields);
                }
                if let Some(uvars) = &result.union_variants {
                    for uv in uvars {
                        if let Some(fields) = uv.type_def.fields.as_ref() {
                            scan(fields);
                        }
                    }
                }
            }
        }

        // Add imports
        out.push_str("use serde::{Deserialize, Serialize};\n");

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
                // Top-level wrappers (for map/array/primitive results) may reference
                // collection types even when there are no object fields to scan.
                let root_rust_type = self.map_ir_type_to_rust(&result, "result", None);
                if root_rust_type.contains("BTreeMap") {
                    needs_btreemap = true;
                }
                if root_rust_type.contains("HashMap") {
                    needs_hashmap = true;
                }
                let mut scan_fields = |fields: &[ir::FieldDef]| {
                    for field in fields {
                        let rust_type = self.map_ir_type_to_rust(
                            &field.field_type,
                            &field.key.as_ident(),
                            None,
                        );
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
                        if rust_type == "BitcoinTransaction"
                            || field_type.contains("bitcoin::Transaction")
                        {
                            needs_transaction = true;
                        }
                        if rust_type == "bitcoin::TxOut" || rust_type.contains("bitcoin::TxOut") {
                            needs_txout = true;
                        }
                        if field_type.contains("TapTree") {
                            needs_taptree = true;
                        }
                        if field_type.contains("ProprietaryKey") {
                            needs_proprietarykey = true;
                        }
                        if rust_type == "bitcoin::Amount" || rust_type.contains("bitcoin::Amount") {
                            needs_amount_deserializer = true;
                        }
                    }
                };
                if let Some(fields) = &result.fields {
                    scan_fields(fields);
                }
                if let Some(uvars) = &result.union_variants {
                    let mut union_needs_btreemap = false;
                    for uv in uvars {
                        if let Some(fields) = uv.type_def.fields.as_ref() {
                            scan_fields(fields);
                        }
                        let rust_ty = self.map_union_variant_rust_type(&uv.type_def, None);
                        if rust_ty.contains("BTreeMap") {
                            union_needs_btreemap = true;
                        }
                        if rust_ty.contains("bitcoin::Txid") {
                            record_external_symbol("bitcoin", "Txid");
                        }
                    }
                    if union_needs_btreemap {
                        needs_btreemap = true;
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
        let union_embedded_object_names = self.collect_union_embedded_object_struct_names(methods);

        let mut processed_types = BTreeSet::new();

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

                // Union verbose branches are emitted by `generate_union_rpc_response` with the
                // correct `rpc_name` for overrides and `should_skip_field_in_struct`. The registry
                // path uses `generate_struct_from_type_def` with an empty `rpc_name`, which would
                // duplicate the type and produce the wrong field types (e.g. getblocktemplate).
                if union_embedded_object_names.contains(nested_type) {
                    processed_types.insert(nested_type.clone());
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

        // Generate response structs for each method
        let mut has_any_responses = false;
        for method in methods {
            set_current_rpc_method(Some(&method.name));
            if let Some(response_struct) = self.generate_method_response(method)? {
                if !has_any_responses {
                    has_any_responses = true;
                }
                out.push_str(&response_struct);
                out.push('\n');
            }
            set_current_rpc_method(None);
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
        /// Register every named object/union under this node. The method's **root** `TypeKind::Union`
        /// (e.g. `GetBlockResponse`) is skipped so it is only emitted by `generate_method_response`,
        /// not as a duplicate from `generate_nested_type`.
        fn visit_method_result_root(ty: &TypeDef, reg: &mut BTreeMap<String, TypeDef>) {
            if matches!(ty.kind, TypeKind::Union) {
                if let Some(uvars) = &ty.union_variants {
                    for uv in uvars {
                        visit_type_for_registry(&uv.type_def, reg);
                    }
                }
                return;
            }
            visit_type_for_registry(ty, reg);
        }

        fn register_named_type(ty: &TypeDef, reg: &mut BTreeMap<String, TypeDef>) {
            let label = ty.rust_emit_name();
            if label.is_empty() || label == "object" || label == "array" {
                return;
            }
            let key = sanitize_type_name_for_rust(label);
            match ty.kind {
                TypeKind::Object => {
                    let replace = match reg.get(&key) {
                        None => true,
                        // Nested maps are visited before their leaf object. First-wins would keep
                        // the map and codegen would emit `pub type Foo = BTreeMap<String, Foo>`.
                        Some(existing) if existing.kind == TypeKind::Map => true,
                        Some(_) => false,
                    };
                    if replace {
                        reg.insert(key, ty.clone());
                    }
                }
                TypeKind::Map | TypeKind::Union => {
                    reg.entry(key).or_insert_with(|| ty.clone());
                }
                _ => {}
            }
        }

        fn visit_type_for_registry(ty: &TypeDef, reg: &mut BTreeMap<String, TypeDef>) {
            if matches!(ty.kind, TypeKind::Object | TypeKind::Map | TypeKind::Union) {
                register_named_type(ty, reg);
            }
            if let Some(uvars) = &ty.union_variants {
                for uv in uvars {
                    visit_type_for_registry(&uv.type_def, reg);
                }
            }
            if let Some(fields) = &ty.fields {
                for f in fields {
                    visit_type_for_registry(&f.field_type, reg);
                }
            }
            if let Some(mv) = ty.map_value.as_deref() {
                visit_type_for_registry(mv, reg);
            }
        }

        for method in methods {
            if let Some(ref result) = method.result {
                visit_method_result_root(result, &mut reg);
            }
        }
        reg
    }

    /// Struct names emitted inline next to **any** `TypeKind::Union` (root or nested). The generic
    /// nested-type pass must skip these so they are not regenerated from `type_registry` without
    /// union-specific merge/skip rules.
    fn collect_union_embedded_object_struct_names(&self, methods: &[RpcDef]) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for method in methods {
            let Some(result) = method.result.as_ref() else {
                continue;
            };
            let result = self.filter_type_def_for_version(result);
            self.union_branch_object_labels_visit(&result, &mut out);
        }
        out
    }

    fn union_branch_object_labels_visit(&self, ty: &TypeDef, out: &mut BTreeSet<String>) {
        if ty.kind == TypeKind::Union {
            if let Some(uvs) = ty.union_variants.as_ref() {
                for uv in uvs {
                    let td = &uv.type_def;
                    match td.kind {
                        TypeKind::Object => {
                            let filtered = self.filter_type_def_for_version(td);
                            out.insert(Self::ir_rust_type_label(&filtered));
                        }
                        TypeKind::Array =>
                            if let Some(elem) = td.array_element_type() {
                                let elabel = elem.rust_emit_name();
                                if matches!(elem.kind, TypeKind::Object)
                                    && !elabel.is_empty()
                                    && elabel != "object"
                                    && elabel != "array"
                                {
                                    let filtered = self.filter_type_def_for_version(elem);
                                    out.insert(Self::ir_rust_type_label(&filtered));
                                }
                            },
                        // Map value objects use the same unqualified `map_ir_type_to_rust` names as
                        // standalone map aliases and `generate_nested_type`; qualifying them here
                        // produced enums pointing at helper structs that were skipped as
                        // "union-embedded" while aliases still referenced the unqualified name.
                        _ => {}
                    }
                }
            }
        }
        if let Some(uvs) = &ty.union_variants {
            for uv in uvs {
                self.union_branch_object_labels_visit(&uv.type_def, out);
            }
        }
        if let Some(fields) = &ty.fields {
            for f in fields {
                self.union_branch_object_labels_visit(&f.field_type, out);
            }
        }
        if let Some(mv) = ty.map_value.as_deref() {
            self.union_branch_object_labels_visit(mv, out);
        }
    }

    /// Rust type string for a union variant (arrays, maps, objects, primitives).
    ///
    /// `parent_enum` is the `#[serde(untagged)]` enum being generated; when set, object (and
    /// array/map) branch structs are qualified with it so names do not collide across unions.
    fn map_union_variant_rust_type(&self, td: &ir::TypeDef, parent_enum: Option<&str>) -> String {
        match td.kind {
            ir::TypeKind::Primitive => self.map_ir_type_to_rust(td, "wire", None),
            ir::TypeKind::Object => {
                let label = td.rust_emit_name();
                if !label.is_empty() && label != "object" && label != "array" {
                    let base = sanitize_type_name_for_rust(label);
                    parent_enum.map(|e| Self::qualify_union_branch_struct(e, &base)).unwrap_or(base)
                } else {
                    record_fallback_event(FallbackEvent {
                        rpc_method: current_rpc_method_or_unknown(),
                        schema_or_ir_path: format!("union_variant:{}", td.name),
                        fallback_kind: "opaque_object_fallback".to_string(),
                        chosen_rust_type: "serde_json::Map<String, serde_json::Value>".to_string(),
                        reason: "unnamed_union_object_variant".to_string(),
                        severity: "P1".to_string(),
                    });
                    "serde_json::Map<String, serde_json::Value>".to_string()
                }
            }
            ir::TypeKind::Array => {
                let elem = td
                    .array_element_type()
                    .expect("array union variant must carry array_element_type");
                format!("Vec<{}>", self.map_union_variant_rust_type(elem, parent_enum))
            }
            ir::TypeKind::Map => {
                let key_ty = match td.map_key_protocol_type.as_deref() {
                    Some("hex") => "bitcoin::Txid",
                    Some("string") | None => "String",
                    Some(unk) => {
                        record_fallback_event(FallbackEvent {
                            rpc_method: current_rpc_method_or_unknown(),
                            schema_or_ir_path: format!("union_variant_map_key:{}", td.name),
                            fallback_kind: "map_string_key_fallback".to_string(),
                            chosen_rust_type: "String".to_string(),
                            reason: format!("unmapped_map_key_protocol_type:{unk}"),
                            severity: "P1".to_string(),
                        });
                        "String"
                    }
                };
                let val = td.map_value_type().expect("map union variant must have map_value");
                let val_rust = match val.kind {
                    // Align with `map_ir_type_to_rust` / nested struct emission (see union_embedded
                    // handling for map branches — value objects are not merged as qualified helpers).
                    ir::TypeKind::Object => self.map_ir_type_to_rust(val, "map_value", None),
                    _ => self.map_union_variant_rust_type(val, parent_enum),
                };
                format!("BTreeMap<{}, {}>", key_ty, val_rust)
            }
            ir::TypeKind::Union => {
                let label = td.rust_emit_name();
                if !label.is_empty() && label != "object" && label != "array" {
                    sanitize_type_name_for_rust(label)
                } else {
                    record_fallback_event(FallbackEvent {
                        rpc_method: current_rpc_method_or_unknown(),
                        schema_or_ir_path: format!("union_variant_union:{}", td.name),
                        fallback_kind: "json_value_fallback".to_string(),
                        chosen_rust_type: "serde_json::Value".to_string(),
                        reason: "unnamed_nested_union_variant".to_string(),
                        severity: "P1".to_string(),
                    });
                    "serde_json::Value".to_string()
                }
            }
            _ => {
                record_fallback_event(FallbackEvent {
                    rpc_method: current_rpc_method_or_unknown(),
                    schema_or_ir_path: format!("union_variant_other:{}", td.name),
                    fallback_kind: "json_value_fallback".to_string(),
                    chosen_rust_type: "serde_json::Value".to_string(),
                    reason: "unhandled_union_variant_kind".to_string(),
                    severity: "P1".to_string(),
                });
                "serde_json::Value".to_string()
            }
        }
    }

    /// If this result represents a top-level JSON array in IR, return the element type.
    ///
    /// Delegates to `TypeDef::array_element_type()` so that array semantics are
    /// centralized in the IR layer instead of re-encoding `FieldKey` conventions here.
    fn array_element_type_from_ir(result: &ir::TypeDef) -> Option<&ir::TypeDef> {
        result.array_element_type()
    }

    /// Returns true iff the IR field has protocol_type "elision". These are documentation/type placeholders,
    /// not real JSON keys.
    fn is_elision_field(field: &ir::FieldDef) -> bool {
        field.field_type.protocol_type.as_deref() == Some("elision")
    }

    /// Returns true iff the field should be skipped when emitting a struct field for the given RPC.
    /// This includes elision placeholders and method-specific scaffolding fields that are not real
    /// JSON keys in Core's responses.
    fn should_skip_field_in_struct(field: &ir::FieldDef) -> bool {
        if Self::is_elision_field(field) {
            return true;
        }
        if field.emit_in_struct == Some(false) {
            return true;
        }
        false
    }

    /// Emit a plain object response struct (standard `Deserialize`), for union variants.
    fn emit_object_response_struct(
        &self,
        result: &TypeDef,
        struct_name: &str,
        rpc_name: &str,
        buf: &mut String,
    ) -> Result<()> {
        let doc = if rpc_name.is_empty() {
            "Object shape from an OpenRPC union branch (nested field)".to_string()
        } else {
            format!("Verbose JSON object (union variant) for `{}` RPC", rpc_name)
        };
        write_doc_line(buf, &doc, "")?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(
            buf,
            "#[cfg_attr(feature = \"serde-deny-unknown-fields\", serde(deny_unknown_fields))]"
        )?;
        writeln!(buf, "pub struct {} {{", struct_name)?;
        if let Some(fields) = &result.fields {
            for field in fields.iter().filter(|f| !Self::should_skip_field_in_struct(f)) {
                self.generate_ir_field(buf, field, struct_name, rpc_name, false)?;
            }
        }
        writeln!(buf, "}}")?;
        writeln!(buf)?;
        Ok(())
    }

    /// When multiple union variants embed the same named object (e.g. `getorphantxs` verbose 1 vs 2
    /// both use `GetOrphanTxsElement` with different fields), merge into one struct: fields that do
    /// not appear in every variant become optional in Rust.
    fn merge_object_type_defs_for_union_branches(
        &self,
        type_defs: &[TypeDef],
        _rpc_name: &str,
    ) -> TypeDef {
        assert!(!type_defs.is_empty());
        let n = type_defs.len();
        let mut merged = type_defs[0].clone();
        if n == 1 {
            return merged;
        }

        let mut ordered_keys: Vec<String> = Vec::new();
        let mut seen = BTreeSet::new();
        for td in type_defs {
            if let Some(fields) = &td.fields {
                for f in fields {
                    if Self::should_skip_field_in_struct(f) {
                        continue;
                    }
                    let k = f.key.as_ident();
                    if seen.insert(k.clone()) {
                        ordered_keys.push(k);
                    }
                }
            }
        }

        let mut merged_fields = Vec::new();
        for key in ordered_keys {
            let mut present: Vec<&ir::FieldDef> = Vec::new();
            for td in type_defs {
                if let Some(fields) = td.fields.as_deref() {
                    if let Some(f) = fields
                        .iter()
                        .find(|f| f.key.as_ident() == key && !Self::should_skip_field_in_struct(f))
                    {
                        present.push(f);
                    }
                }
            }
            let in_all = present.len() == n;
            let Some(template) = present.first().copied() else {
                continue;
            };
            let mut mf = (*template).clone();
            mf.required = in_all && present.iter().all(|f| f.required);
            merged_fields.push(mf);
        }
        merged.fields = Some(merged_fields);
        merged
    }

    /// JSON-RPC result is a top-level alternation (e.g. hex string vs object): `#[serde(untagged)]` enum.
    fn generate_union_rpc_response(
        &self,
        method: &RpcDef,
        union_td: &TypeDef,
        enum_name: &str,
    ) -> Result<String> {
        let mut buf = String::new();
        let canonical_name =
            crate::utils::canonical_from_adapter_method(&self.implementation, &method.name, None)
                .unwrap_or_else(|_| enum_name.replace("Response", ""));
        write_doc_line(&mut buf, &format!("Response for the `{}` RPC method", canonical_name), "")?;
        buf.push_str(&self.emit_union_definition(union_td, enum_name, method.name.as_str())?);
        Ok(buf)
    }

    /// Emit merged branch structs and a `#[serde(untagged)]` enum for any IR `TypeKind::Union`.
    /// `rpc_doc_name` is used in struct doc comments; use `""` for nested unions.
    fn emit_union_definition(
        &self,
        union_td: &TypeDef,
        enum_name: &str,
        rpc_doc_name: &str,
    ) -> Result<String> {
        let union_td = self.filter_type_def_for_version(union_td);
        let uvs =
            union_td.union_variants.as_ref().expect("union response must have union_variants");
        let mut variants: Vec<&UnionVariantDef> = uvs.iter().collect();
        variants.sort_by_key(|v| match v.type_def.kind {
            TypeKind::Primitive => 0,
            TypeKind::Array => 1,
            TypeKind::Map => 2,
            TypeKind::Union => 3,
            TypeKind::Object => 4,
            _ => 5,
        });

        let mut buf = String::new();

        // Group inner object shapes by Rust struct name so we emit each helper struct once (merged
        // when union branches reuse the same IR type name with different fields).
        let mut inner_by_name: BTreeMap<String, Vec<TypeDef>> = BTreeMap::new();
        for uv in &variants {
            let td = &uv.type_def;
            match td.kind {
                TypeKind::Object => {
                    let filtered = self.filter_type_def_for_version(td);
                    let inner_name = Self::ir_rust_type_label(&filtered);
                    inner_by_name.entry(inner_name).or_default().push(filtered);
                }
                TypeKind::Array =>
                    if let Some(elem) = td.array_element_type() {
                        if matches!(elem.kind, TypeKind::Object) {
                            let filtered = self.filter_type_def_for_version(elem);
                            let inner_name = Self::ir_rust_type_label(&filtered);
                            inner_by_name.entry(inner_name).or_default().push(filtered);
                        }
                    },
                _ => {}
            }
        }
        for (inner_name, defs) in inner_by_name {
            let merged = self.merge_object_type_defs_for_union_branches(&defs, rpc_doc_name);
            let qualified = Self::qualify_union_branch_struct(enum_name, &inner_name);
            self.emit_object_response_struct(&merged, &qualified, rpc_doc_name, &mut buf)?;
        }

        writeln!(buf, "#[allow(clippy::large_enum_variant)]")?;
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]")?;
        writeln!(buf, "#[serde(untagged)]")?;
        writeln!(buf, "pub enum {} {{", enum_name)?;
        for uv in &variants {
            let variant_name = sanitize_type_name_for_rust(&uv.name);
            let rust_ty = self.map_union_variant_rust_type(&uv.type_def, Some(enum_name));
            writeln!(buf, "    {}({}),", variant_name, rust_ty)?;
        }
        writeln!(buf, "}}")?;
        Ok(buf)
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

        if let Some(result) = &method.result {
            let r = self.filter_type_def_for_version(result);
            if r.kind == TypeKind::Primitive && r.protocol_type.as_deref() == Some("any") {
                let struct_name = self.response_struct_name(method);
                return Ok(Some(self.generate_value_wrapper(method, &struct_name)?));
            }
            if r.kind == TypeKind::Map {
                let struct_name = self.response_struct_name(method);
                return Ok(Some(self.generate_map_wrapper(method, &struct_name, &r)?));
            }
            if r.kind == TypeKind::Union {
                if let Some(ref uvs) = r.union_variants {
                    if !uvs.is_empty() {
                        let struct_name = self.response_struct_name(method);
                        return Ok(Some(self.generate_union_rpc_response(
                            method,
                            &r,
                            &struct_name,
                        )?));
                    }
                }
            }
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

    /// Generate a transparent wrapper for top-level map results.
    fn generate_map_wrapper(
        &self,
        method: &RpcDef,
        struct_name: &str,
        result: &ir::TypeDef,
    ) -> Result<String> {
        let mut buf = String::new();
        let canonical_name =
            crate::utils::canonical_from_adapter_method(&self.implementation, &method.name, None)
                .unwrap_or_else(|_| struct_name.replace("Response", ""));
        write_doc_line(&mut buf, &format!("Response for the `{}` RPC method", canonical_name), "")?;
        writeln!(&mut buf, "///")?;
        write_doc_line(
            &mut buf,
            "This method returns a dynamic-key object wrapped in a transparent struct.",
            "",
        )?;
        let map_ty = self.map_ir_type_to_rust(result, "result", Some(struct_name));
        writeln!(&mut buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(&mut buf, "#[serde(transparent)]")?;
        writeln!(&mut buf, "pub struct {}(pub {});", struct_name, map_ty)?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "impl std::ops::Deref for {} {{", struct_name)?;
        writeln!(&mut buf, "    type Target = {};", map_ty)?;
        writeln!(&mut buf, "    fn deref(&self) -> &Self::Target {{ &self.0 }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "impl std::ops::DerefMut for {} {{", struct_name)?;
        writeln!(&mut buf, "    fn deref_mut(&mut self) -> &mut Self::Target {{ &mut self.0 }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "impl From<{}> for {} {{", map_ty, struct_name)?;
        writeln!(&mut buf, "    fn from(value: {}) -> Self {{ Self(value) }}", map_ty)?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;
        writeln!(&mut buf, "impl From<{}> for {} {{", struct_name, map_ty)?;
        writeln!(&mut buf, "    fn from(wrapper: {}) -> Self {{ wrapper.0 }}", struct_name)?;
        writeln!(&mut buf, "}}")?;
        Ok(buf)
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

        // Root-shape alternation (wire primitives vs objects) is represented in IR as
        // `TypeKind::Union` and emitted by `generate_union_rpc_response`. Plain object
        // responses are always mechanical `Deserialize` derives here.
        writeln!(&mut buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(
            &mut buf,
            "#[cfg_attr(feature = \"serde-deny-unknown-fields\", serde(deny_unknown_fields))]"
        )?;
        writeln!(&mut buf, "pub struct {} {{", struct_name)?;

        // Generate fields from IR data.
        if let Some(fields) = &result.fields {
            for field in fields.iter().filter(|f| !Self::should_skip_field_in_struct(f)) {
                self.generate_ir_field(&mut buf, field, &struct_name, method.name.as_str(), false)?;
            }
        }

        writeln!(&mut buf, "}}")?;

        Ok(buf)
    }

    /// Emit a single struct from an IR `TypeDef` (for nested/named types from
    /// the type registry). Uses the same field logic as method responses; no
    /// `deny_unknown_fields` on inner structs. For Bitcoin Core the type is
    /// first filtered for the generator's target version so nested helpers do
    /// not include fields that are not present in this release.
    fn generate_struct_from_type_def(&self, type_def: &TypeDef, buf: &mut String) -> Result<()> {
        let type_def = self.filter_type_def_for_version(type_def);
        let struct_name = Self::ir_rust_type_label(&type_def);
        if !type_def.description.is_empty() {
            write_doc_comment(buf, &type_def.description, "")?;
        }
        writeln!(buf, "#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]")?;
        writeln!(buf, "pub struct {} {{", struct_name)?;
        if let Some(fields) = &type_def.fields {
            for field in fields.iter().filter(|f| !Self::is_elision_field(f)) {
                self.generate_ir_field(buf, field, &struct_name, "", false)?;
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
        struct_name: &str,
        _rpc_name: &str,
        force_optional_conditional: bool,
    ) -> Result<()> {
        // Generate field documentation
        if !field.description.is_empty() {
            write_doc_comment(buf, &field.description, "    ")?;
        }

        // Generate field definition from IR.
        let mut base_field_type =
            self.map_ir_type_to_rust(&field.field_type, &field.key.as_ident(), Some(struct_name));
        // Direct self-recursion (e.g. getaddressinfo `embedded`) needs heap indirection for a
        // known size; otherwise Rust reports E0072 / layout cycles.
        if !struct_name.is_empty() && base_field_type == struct_name {
            base_field_type = format!("Box<{struct_name}>");
        }
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
        // IR: `force_optional` when Core may omit despite `required` in schema.
        let force_opt = field.force_optional == Some(true);
        if force_opt {
            field_type = format!("Option<{}>", base_field_type);
        }

        // Add serde rename attribute if the field name was changed
        if field_name != field.key.as_ident() {
            writeln!(buf, "    #[serde(rename = \"{}\")]", field.key.as_ident())?;
        }

        let amount_option =
            base_field_type == "bitcoin::Amount" && field_type.starts_with("Option<");
        // `force_opt` uses `#[serde(default)]`; Option + Amount merges `default` into the Amount attribute below.
        if force_opt && !amount_option {
            writeln!(buf, "    #[serde(default)]")?;
        }

        // Add deserializer attribute for bitcoin::Amount fields
        // Check if the base type (before Option wrapper) is bitcoin::Amount
        if base_field_type == "bitcoin::Amount" {
            // Use different deserializer for Option<Amount> vs Amount.
            // `default` is required for Option + `deserialize_with` so a missing JSON field deserializes as None.
            if field_type.starts_with("Option<") {
                writeln!(
                    buf,
                    "    #[serde(default, deserialize_with = \"option_amount_from_btc_float\")]"
                )?;
            } else {
                writeln!(buf, "    #[serde(deserialize_with = \"amount_from_btc_float\")]")?;
            }
        }

        writeln!(buf, "    pub {}: {},", field_name, field_type)?;
        Ok(())
    }

    /// Map IR TypeDef to Rust type
    fn map_ir_type_to_rust(
        &self,
        type_def: &ir::TypeDef,
        field_name: &str,
        enclosing_struct: Option<&str>,
    ) -> String {
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
                if let Some(elem) = type_def.homogeneous_array_element_type() {
                    return format!(
                        "Vec<{}>",
                        self.map_ir_type_to_rust(elem, field_name, enclosing_struct)
                    );
                }
                if type_def.prefix_items_tuple_fields().is_some() {
                    return "Vec<serde_json::Value>".to_string();
                }
                // Fallback when IR omits element metadata.
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
                if rust_type == "Vec<String>" {
                    record_fallback_event(FallbackEvent {
                        rpc_method: current_rpc_method_or_unknown(),
                        schema_or_ir_path: format!("array_field:{}", field_name),
                        fallback_kind: "array_value_fallback".to_string(),
                        chosen_rust_type: rust_type.to_string(),
                        reason: "array_missing_element_metadata".to_string(),
                        severity: "P1".to_string(),
                    });
                }
                rust_type.to_string()
            }
            ir::TypeKind::Map => {
                let key_ty = match type_def.map_key_protocol_type.as_deref() {
                    Some("hex") => "bitcoin::Txid",
                    Some("string") | None =>
                    // Core metadata often omits key type for conventional string-keyed objects; String is correct.
                        "String",
                    Some(unk) => {
                        record_fallback_event(FallbackEvent {
                            rpc_method: current_rpc_method_or_unknown(),
                            schema_or_ir_path: format!("map_field:{}", field_name),
                            fallback_kind: "map_string_key_fallback".to_string(),
                            chosen_rust_type: "String".to_string(),
                            reason: format!("unmapped_map_key_protocol_type:{unk}"),
                            severity: "P1".to_string(),
                        });
                        "String"
                    }
                };
                let val =
                    type_def.map_value_type().expect("TypeKind::Map must set map_value in IR");
                format!(
                    "BTreeMap<{}, {}>",
                    key_ty,
                    self.map_ir_type_to_rust(val, field_name, enclosing_struct)
                )
            }
            ir::TypeKind::Union => {
                let label = type_def.rust_emit_name();
                if !label.is_empty() && label != "object" && label != "array" {
                    let rust_name = sanitize_type_name_for_rust(label);
                    if enclosing_struct.is_some_and(|s| s == rust_name.as_str()) {
                        format!("Box<{rust_name}>")
                    } else {
                        rust_name
                    }
                } else {
                    record_fallback_event(FallbackEvent {
                        rpc_method: current_rpc_method_or_unknown(),
                        schema_or_ir_path: format!("union_field:{}", field_name),
                        fallback_kind: "json_value_fallback".to_string(),
                        chosen_rust_type: "serde_json::Value".to_string(),
                        reason: "unnamed_union_field_type".to_string(),
                        severity: "P1".to_string(),
                    });
                    "serde_json::Value".to_string()
                }
            }
            ir::TypeKind::Object => {
                // Decoded tx fields: IR uses Object (with nested array shape) for vin/vout; map to typed vecs.
                // For dynamic per-message byte stats (OBJ_DYN with numeric values), map to a typed map
                // rather than a generic JSON value when the IR encodes an object with a single "msg"
                // field of numeric primitive type.
                if let Some(fields) = &type_def.fields {
                    if fields.len() == 1 {
                        let f = &fields[0];
                        if f.key.as_ident() == "msg"
                            && matches!(f.field_type.kind, ir::TypeKind::Primitive)
                            && f.field_type.protocol_type.as_deref() == Some("number")
                        {
                            return "std::collections::HashMap<String, u64>".to_string();
                        }
                    }
                }

                // Arrays encoded as objects with `protocol_type: "array"` and a single
                // nested element field (as used by decodepsbt-style responses).
                if type_def.protocol_type.as_deref() == Some("array") {
                    if let Some(fields) = &type_def.fields {
                        if fields.len() == 1 {
                            let outer = &fields[0].field_type;
                            if let Some(outer_fields) = &outer.fields {
                                if outer_fields.len() == 1 {
                                    let elem = &outer_fields[0].field_type;
                                    // When the innermost element has a concrete name
                                    // (e.g. DecodepsbtInput / DecodepsbtOutput), map to
                                    // a typed Vec rather than serde_json::Value.
                                    let elabel = elem.rust_emit_name();
                                    if !elabel.is_empty() && elabel != "object" && elabel != "array"
                                    {
                                        return format!(
                                            "Vec<{}>",
                                            sanitize_type_name_for_rust(elabel)
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                // Named object types (non-generic) should map to their own struct
                // instead of falling back to serde_json::Value. This allows IR-
                // defined nested types like DecodePsbtTx to propagate into the
                // generated response struct.
                let olabel = type_def.rust_emit_name();
                if !olabel.is_empty() && olabel != "object" && olabel != "array" {
                    let rust_name = sanitize_type_name_for_rust(olabel);
                    return if enclosing_struct.is_some_and(|s| s == rust_name.as_str()) {
                        format!("Box<{rust_name}>")
                    } else {
                        rust_name
                    };
                }

                match field_name {
                    "vin" => "Vec<DecodedVin>".to_string(),
                    "vout" => "Vec<DecodedVout>".to_string(),
                    _ => {
                        record_fallback_event(FallbackEvent {
                            rpc_method: current_rpc_method_or_unknown(),
                            schema_or_ir_path: format!("object_field:{}", field_name),
                            fallback_kind: "opaque_object_fallback".to_string(),
                            chosen_rust_type: "serde_json::Map<String, serde_json::Value>"
                                .to_string(),
                            reason: "generic_object_without_named_shape".to_string(),
                            severity: "P1".to_string(),
                        });
                        "serde_json::Map<String, serde_json::Value>".to_string()
                    }
                }
            }
            _ => {
                record_fallback_event(FallbackEvent {
                    rpc_method: current_rpc_method_or_unknown(),
                    schema_or_ir_path: format!("type_kind_other:{}", field_name),
                    fallback_kind: "json_value_fallback".to_string(),
                    chosen_rust_type: "serde_json::Value".to_string(),
                    reason: "unhandled_type_kind".to_string(),
                    severity: "P1".to_string(),
                });
                "serde_json::Value".to_string()
            }
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
            record_fallback_event(FallbackEvent {
                rpc_method: current_rpc_method_or_unknown(),
                schema_or_ir_path: format!("metadata_type:{type_name}"),
                fallback_kind: "map_value_fallback".to_string(),
                chosen_rust_type: fixed.clone(),
                reason: "metadata_incomplete_hashmap_type".to_string(),
                severity: "P2".to_string(),
            });
            return fixed;
        }

        // Fix specific case: Option<HashMap<String -> Option<HashMap<String, serde_json::Value>>
        if type_name == "Option<HashMap<String" {
            let fixed = "Option<HashMap<String, serde_json::Value>>".to_string();
            record_fallback_event(FallbackEvent {
                rpc_method: current_rpc_method_or_unknown(),
                schema_or_ir_path: "metadata_type:Option<HashMap<String".to_string(),
                fallback_kind: "map_value_fallback".to_string(),
                chosen_rust_type: fixed.clone(),
                reason: "metadata_malformed_hashmap_option".to_string(),
                severity: "P2".to_string(),
            });
            return fixed;
        }
        if type_name.contains("BTreeMap<String") && !type_name.contains(',') {
            // Handle cases like "Option<BTreeMap<String" or "BTreeMap<String"
            let fixed = type_name.replace("BTreeMap<String", "BTreeMap<String, serde_json::Value>");
            record_fallback_event(FallbackEvent {
                rpc_method: current_rpc_method_or_unknown(),
                schema_or_ir_path: format!("metadata_type:{type_name}"),
                fallback_kind: "map_value_fallback".to_string(),
                chosen_rust_type: fixed.clone(),
                reason: "metadata_incomplete_btreemap_type".to_string(),
                severity: "P2".to_string(),
            });
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
        if rust_type == "serde_json::Value" {
            record_fallback_event(FallbackEvent {
                rpc_method: current_rpc_method_or_unknown(),
                schema_or_ir_path: format!("metadata_type:{type_name}"),
                fallback_kind: "json_value_fallback".to_string(),
                chosen_rust_type: rust_type.to_string(),
                reason: "metadata_type_collapsed_to_any".to_string(),
                severity: "P1".to_string(),
            });
        }
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
        let value_ty = if let Some(elem_ty) = result.homogeneous_array_element_type() {
            // When the IR describes an array of objects with a named element
            // type (e.g. for top-level array RPCs such as getpeerinfo), use
            // that named type so changes to the OpenRPC element schema are
            // reflected in a dedicated struct instead of being erased to
            // `serde_json::Value`.
            let elabel = elem_ty.rust_emit_name();
            if matches!(elem_ty.kind, ir::TypeKind::Object)
                && !elabel.is_empty()
                && elabel != "object"
            {
                Self::ir_rust_type_label(elem_ty)
            } else {
                self.map_ir_type_to_rust(elem_ty, "field_0", None)
            }
        } else {
            record_fallback_event(FallbackEvent {
                rpc_method: method.name.clone(),
                schema_or_ir_path: format!("result:{}:array_element", method.name),
                fallback_kind: "array_value_fallback".to_string(),
                chosen_rust_type: "Vec<serde_json::Value>".to_string(),
                reason: "top_level_array_missing_element_metadata".to_string(),
                severity: "P1".to_string(),
            });
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
    ///
    /// `is_method_result_root`: when true, the node is the RPC's top-level `result` type. That root
    /// must not be listed if it is `TypeKind::Union` (`GetBlockResponse`, …), because that enum is
    /// emitted by `generate_method_response`, not `generate_nested_type`.
    fn collect_nested_types_from_type_def(
        &self,
        type_def: &ir::TypeDef,
        nested_types: &mut BTreeSet<String>,
        is_method_result_root: bool,
    ) {
        let is_union = type_def.kind == TypeKind::Union;
        if is_union && !is_method_result_root {
            let label = type_def.rust_emit_name();
            if !label.is_empty() && label != "object" && label != "array" {
                nested_types.insert(sanitize_type_name_for_rust(label));
            }
        }

        if !is_union {
            let label = type_def.rust_emit_name();
            // `rust_emit_name()` prefers `type_identity` (OpenRPC), which is often snake_case.
            // Only track shaped kinds here—primitives like `string` would sanitize to `String`
            // and pollute the nested pass if we keyed purely on "starts with uppercase".
            if matches!(type_def.kind, TypeKind::Object | TypeKind::Map)
                && !label.is_empty()
                && label != "object"
                && label != "array"
            {
                nested_types.insert(sanitize_type_name_for_rust(label));
            }
            self.collect_nested_types(label, nested_types);
        }
        if let Some(ref uvars) = type_def.union_variants {
            for uv in uvars {
                self.collect_nested_types_from_type_def(&uv.type_def, nested_types, false);
            }
        }
        if let Some(ref fields) = type_def.fields {
            for field in fields {
                self.collect_nested_types_from_type_def(&field.field_type, nested_types, false);
            }
        }
        if let Some(mv) = type_def.map_value.as_deref() {
            self.collect_nested_types_from_type_def(mv, nested_types, false);
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
                        nested_types.insert(sanitize_type_name_for_rust(word));
                    }
                }
            }
        }
    }

    /// Generate a nested type: from type registry when present, else skip (decoded-tx) or type alias.
    fn generate_nested_type(
        &self,
        type_name: &str,
        type_registry: &BTreeMap<String, TypeDef>,
    ) -> Result<Option<String>> {
        let lookup_name = sanitize_type_name_for_rust(type_name);
        if let Some(type_def) = type_registry.get(&lookup_name) {
            if matches!(type_def.kind, TypeKind::Object) && type_def.fields.is_some() {
                let mut buf = String::new();
                self.generate_struct_from_type_def(type_def, &mut buf)?;
                return Ok(Some(buf));
            }
            if matches!(type_def.kind, TypeKind::Map) {
                let mut output = String::new();
                let rust_type = self.map_ir_type_to_rust(type_def, &lookup_name, None);
                write_doc_line(&mut output, &format!("Type alias for {}", type_name), "")?;
                writeln!(
                    output,
                    "pub type {} = {};",
                    sanitize_type_name_for_rust(type_name),
                    rust_type
                )?;
                return Ok(Some(output));
            }
            if matches!(type_def.kind, TypeKind::Union)
                && type_def.union_variants.as_ref().is_some_and(|v| !v.is_empty())
            {
                return Ok(Some(self.emit_union_definition(type_def, &lookup_name, "")?));
            }
        }
        let type_alias = self.generate_type_alias(type_name)?;
        Ok(Some(type_alias))
    }

    /// Generate a type alias for types not found in metadata
    fn generate_type_alias(&self, type_name: &str) -> Result<String> {
        if type_name.ends_with("ResultMap") {
            return Ok(String::new());
        }
        // Check if this type is already imported from external crates
        if self.is_external_type(type_name) {
            return Ok(String::new());
        }

        // Map common Bitcoin Core types to their appropriate Rust types
        let rust_type = match type_name {
            "Bip125Replaceable" => "bool", // Boolean for replaceable status
            "ScriptPubkey" => "bitcoin::ScriptBuf", // Hex-encoded script pubkey
            _ => {
                let normalized = type_name.to_ascii_lowercase();
                match normalized.as_str() {
                    "amount" => "bitcoin::Amount",
                    "blockhash" => "bitcoin::BlockHash",
                    "txid" => "bitcoin::Txid",
                    _ => "String",
                }
            }
        };

        if rust_type == "String" {
            record_fallback_event(FallbackEvent {
                rpc_method: current_rpc_method_or_unknown(),
                schema_or_ir_path: format!("type_alias:{type_name}"),
                fallback_kind: "unknown_alias_string_fallback".to_string(),
                chosen_rust_type: "String".to_string(),
                reason: "unmapped_alias_default_string".to_string(),
                severity: "P2".to_string(),
            });
        }

        let mut output = String::new();
        write_doc_line(&mut output, &format!("Type alias for {}", type_name), "")?;
        writeln!(output, "pub type {} = {};", sanitize_type_name_for_rust(type_name), rust_type)?;
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
    fn scalar_type_aliases_use_bitcoin_types() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        let amount_alias = gen.generate_type_alias("amount").unwrap();
        assert!(amount_alias.contains("pub type Amount = bitcoin::Amount;"));

        let blockhash_alias = gen.generate_type_alias("BlockHash").unwrap();
        assert!(blockhash_alias.contains("pub type BlockHash = bitcoin::BlockHash;"));

        // Also accept lowercase input; left-side casing is derived from `sanitize_type_name_for_rust`.
        let blockhash_alias_lower = gen.generate_type_alias("blockhash").unwrap();
        assert!(blockhash_alias_lower.contains("pub type Blockhash = bitcoin::BlockHash;"));

        // `Txid` must be mapped (it was previously missing and defaulted to `String`)
        let txid_alias = gen.generate_type_alias("Txid").unwrap();
        assert!(txid_alias.contains("pub type Txid = bitcoin::Txid;"));
    }

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
            ..Default::default()
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
                emit_in_struct: None,
                force_optional: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("array".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let method = RpcDef {
            name: "deriveaddresses".to_string(),
            result: Some(result_ty),
            ..Default::default()
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
            ..Default::default()
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
                emit_in_struct: None,
                force_optional: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("array".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let method = RpcDef {
            name: "deriveaddresses".to_string(),
            result: Some(result_ty),
            ..Default::default()
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

    /// Asserts that decodepsbt-style response uses IR-driven nested types
    /// (DecodePsbtTx, Vec<DecodePsbtInput>, Vec<DecodePsbtOutput>)
    /// rather than serde_json::Value, so schema changes propagate.
    #[test]
    fn decodepsbt_response_uses_ir_nested_types() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        let tx_obj = TypeDef {
            name: "DecodePsbtTx".to_string(),
            description: String::new(),
            kind: TypeKind::Object,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Named("txid".to_string()),
                field_type: TypeDef {
                    name: "hex".to_string(),
                    description: String::new(),
                    kind: TypeKind::Primitive,
                    fields: None,
                    variants: None,
                    union_variants: None,
                    base_type: None,
                    protocol_type: Some("hex".to_string()),
                    canonical_name: None,
                    condition: None,
                    ..Default::default()
                },
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("object".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let empty_object = |name: &str| TypeDef {
            name: name.to_string(),
            description: String::new(),
            kind: TypeKind::Object,
            fields: Some(vec![]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("object".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };
        let input_elem = empty_object("DecodePsbtInput");
        let output_elem = empty_object("DecodePsbtOutput");

        let make_array_of_objects = |elem: TypeDef| TypeDef {
            name: "array".to_string(),
            description: String::new(),
            kind: TypeKind::Object,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Named("field".to_string()),
                field_type: TypeDef {
                    name: "object".to_string(),
                    description: String::new(),
                    kind: TypeKind::Object,
                    fields: Some(vec![ir::FieldDef {
                        key: ir::FieldKey::Named("field_0".to_string()),
                        field_type: elem,
                        required: true,
                        description: String::new(),
                        default_value: None,
                        version_added: None,
                        version_removed: None,
                        emit_in_struct: None,
                        force_optional: None,
                    }]),
                    variants: None,
                    union_variants: None,
                    base_type: None,
                    protocol_type: Some("object".to_string()),
                    canonical_name: None,
                    condition: None,
                    ..Default::default()
                },
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("array".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let inputs_array = make_array_of_objects(input_elem);
        let outputs_array = make_array_of_objects(output_elem);

        let result_ty = TypeDef {
            name: "object".to_string(),
            description: String::new(),
            kind: TypeKind::Object,
            fields: Some(vec![
                ir::FieldDef {
                    key: ir::FieldKey::Named("tx".to_string()),
                    field_type: tx_obj,
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                },
                ir::FieldDef {
                    key: ir::FieldKey::Named("inputs".to_string()),
                    field_type: inputs_array,
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                },
                ir::FieldDef {
                    key: ir::FieldKey::Named("outputs".to_string()),
                    field_type: outputs_array,
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                },
            ]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("object".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let method = RpcDef {
            name: "decodepsbt".to_string(),
            result: Some(result_ty),
            ..Default::default()
        };

        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");

        assert!(
            code.contains("DecodePsbtTx"),
            "decodepsbt response must use IR type DecodePsbtTx, got:\n{code}"
        );
        assert!(
            code.contains("Vec<DecodePsbtInput>"),
            "decodepsbt response must use Vec<DecodePsbtInput> from IR, got:\n{code}"
        );
        assert!(
            code.contains("Vec<DecodePsbtOutput>"),
            "decodepsbt response must use Vec<DecodePsbtOutput> from IR, got:\n{code}"
        );
    }

    #[test]
    fn type_identity_overrides_name_for_rust_emit() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        let input_elem = TypeDef {
            name: "WrongNestedLabel".to_string(),
            type_identity: Some("DecodePsbtInput".to_string()),
            description: String::new(),
            kind: TypeKind::Object,
            fields: Some(vec![]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("object".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let make_array_of_objects = |elem: TypeDef| TypeDef {
            name: "array".to_string(),
            description: String::new(),
            kind: TypeKind::Object,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Named("field".to_string()),
                field_type: TypeDef {
                    name: "object".to_string(),
                    description: String::new(),
                    kind: TypeKind::Object,
                    fields: Some(vec![ir::FieldDef {
                        key: ir::FieldKey::Named("field_0".to_string()),
                        field_type: elem,
                        required: true,
                        description: String::new(),
                        default_value: None,
                        version_added: None,
                        version_removed: None,
                        emit_in_struct: None,
                        force_optional: None,
                    }]),
                    variants: None,
                    union_variants: None,
                    base_type: None,
                    protocol_type: Some("object".to_string()),
                    canonical_name: None,
                    condition: None,
                    ..Default::default()
                },
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("array".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let inputs_array = make_array_of_objects(input_elem);
        let result_ty = TypeDef {
            name: "object".to_string(),
            description: String::new(),
            kind: TypeKind::Object,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Named("inputs".to_string()),
                field_type: inputs_array,
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("object".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let method = RpcDef {
            name: "decodepsbt".to_string(),
            result: Some(result_ty),
            ..Default::default()
        };

        let code = gen
            .generate(std::slice::from_ref(&method))
            .expect("generation must succeed")
            .into_iter()
            .find(|(f, _)| f == "responses.rs")
            .expect("responses.rs")
            .1;

        assert!(
            code.contains("pub struct DecodePsbtInput"),
            "expected struct from type_identity, not WrongNestedLabel; got:\n{code}"
        );
        assert!(
            !code.contains("WrongNestedLabel"),
            "name-only label must not appear when type_identity is set; got:\n{code}"
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
            ..Default::default()
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
                emit_in_struct: None,
                force_optional: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("array".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let method = RpcDef {
            name: "getrawmempool".to_string(),
            result: Some(result_ty),
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
                    emit_in_struct: None,
                    force_optional: Some(true),
                },
                ir::FieldDef {
                    key: ir::FieldKey::Named("version".to_string()),
                    field_type: version_ty,
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                },
            ]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("object".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let method = RpcDef {
            name: "getblocktemplate".to_string(),
            result: Some(result_ty),
            ..Default::default()
        };

        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");

        // IR `force_optional` on the proposal placeholder mirrors canonical `getblocktemplate`.
        assert!(
            code.contains("#[serde(default)]"),
            "expected serde default attr for placeholder field, got:\n{code}"
        );
        assert!(
            code.contains("pub field_0: Option<()>"),
            "expected optional unit placeholder field, got:\n{code}"
        );
    }

    #[test]
    fn getblock_skips_ir_union_suffix_duplicate_fields() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        let hex_ty = TypeDef {
            name: "hex".to_string(),
            description: String::new(),
            kind: TypeKind::Primitive,
            fields: None,
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("hex".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let result_ty = TypeDef {
            name: "object".to_string(),
            description: String::new(),
            kind: TypeKind::Object,
            fields: Some(vec![
                ir::FieldDef {
                    key: ir::FieldKey::Named("hash".to_string()),
                    field_type: hex_ty.clone(),
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                },
                ir::FieldDef {
                    key: ir::FieldKey::Named("hash_1".to_string()),
                    field_type: hex_ty,
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                },
            ]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("object".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let method =
            RpcDef { name: "getblock".to_string(), result: Some(result_ty), ..Default::default() };

        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");

        assert!(
            code.contains("pub hash:"),
            "expected single hash field for wire JSON key `hash`, got:\n{code}"
        );
        assert!(
            code.contains("pub hash_1:"),
            "without codegen fallback skipping, IR disambiguation key hash_1 is emitted unless IR marks emit_in_struct=false, got:\n{code}"
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
                ..Default::default()
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
                    emit_in_struct: None,
                    force_optional: None,
                }]),
                variants: None,
                union_variants: None,
                base_type: None,
                protocol_type: None,
                canonical_name: None,
                condition: None,
                ..Default::default()
            }
        }

        fn rpc_method(name: &str, result: TypeDef) -> RpcDef {
            RpcDef { name: name.to_string(), result: Some(result), ..Default::default() }
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

    #[test]
    fn union_rpc_response_emits_untagged_enum() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());
        let wire = TypeDef {
            name: "string".to_string(),
            kind: TypeKind::Primitive,
            protocol_type: Some("string".to_string()),
            ..Default::default()
        };
        let verbose_inner = TypeDef {
            name: "DemoVerbose".to_string(),
            kind: TypeKind::Object,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Named("hex".to_string()),
                field_type: TypeDef {
                    name: "hex".to_string(),
                    kind: TypeKind::Primitive,
                    protocol_type: Some("hex".to_string()),
                    ..Default::default()
                },
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            protocol_type: Some("object".to_string()),
            ..Default::default()
        };
        let union_td = TypeDef {
            name: "DemoRpcResponse".to_string(),
            kind: TypeKind::Union,
            union_variants: Some(vec![
                ir::UnionVariantDef {
                    name: "DemoWire".to_string(),
                    description: "wire".to_string(),
                    condition: None,
                    type_def: wire,
                },
                ir::UnionVariantDef {
                    name: "DemoVerboseVariant".to_string(),
                    description: "verbose".to_string(),
                    condition: None,
                    type_def: verbose_inner,
                },
            ]),
            protocol_type: Some("union".to_string()),
            ..Default::default()
        };
        let method =
            RpcDef { name: "demo_rpc".to_string(), result: Some(union_td), ..Default::default() };
        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");
        assert!(code.contains("#[serde(untagged)]"), "got:\n{code}");
        assert!(
            code.contains("pub struct DemoRpcResponseDemoVerbose"),
            "branch structs are qualified with parent enum name, got:\n{code}"
        );
        assert!(code.contains("pub enum DemoRpcResponse"), "got:\n{code}");
    }

    /// `getorphantxs`-style unions: two array variants reuse one IR element type name with different
    /// fields; we must emit a single merged struct (extra fields optional).
    #[test]
    fn union_array_branches_merge_same_named_element_struct() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        let str_ty = || TypeDef {
            name: "string".to_string(),
            kind: TypeKind::Primitive,
            protocol_type: Some("string".to_string()),
            ..Default::default()
        };
        let hex_ty = TypeDef {
            name: "hex".to_string(),
            kind: TypeKind::Primitive,
            protocol_type: Some("hex".to_string()),
            ..Default::default()
        };

        let elem_v1 = TypeDef {
            name: "MergedElem".to_string(),
            kind: TypeKind::Object,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Named("txid".to_string()),
                field_type: str_ty(),
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            protocol_type: Some("object".to_string()),
            ..Default::default()
        };

        let elem_v2 = TypeDef {
            name: "MergedElem".to_string(),
            kind: TypeKind::Object,
            fields: Some(vec![
                ir::FieldDef {
                    key: ir::FieldKey::Named("txid".to_string()),
                    field_type: str_ty(),
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                },
                ir::FieldDef {
                    key: ir::FieldKey::Named("hex".to_string()),
                    field_type: hex_ty,
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                },
            ]),
            protocol_type: Some("object".to_string()),
            ..Default::default()
        };

        let array_of = |elem: TypeDef| TypeDef {
            name: "array".to_string(),
            kind: TypeKind::Array,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Anonymous(0),
                field_type: elem,
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            protocol_type: Some("array".to_string()),
            ..Default::default()
        };

        let union_td = TypeDef {
            name: "DemoOrphanStyleResponse".to_string(),
            kind: TypeKind::Union,
            union_variants: Some(vec![
                ir::UnionVariantDef {
                    name: "Branch1".to_string(),
                    description: String::new(),
                    condition: None,
                    type_def: array_of(elem_v1),
                },
                ir::UnionVariantDef {
                    name: "Branch2".to_string(),
                    description: String::new(),
                    condition: None,
                    type_def: array_of(elem_v2),
                },
            ]),
            protocol_type: Some("union".to_string()),
            ..Default::default()
        };

        let method = RpcDef {
            name: "demo_orphan_style".to_string(),
            result: Some(union_td),
            ..Default::default()
        };

        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");

        assert_eq!(
            code.matches("pub struct DemoOrphanStyleResponseMergedElem").count(),
            1,
            "expected exactly one qualified merged element struct, got:\n{code}"
        );
        assert!(
            code.contains("pub hex: Option<String>"),
            "hex only in one branch must be optional, got:\n{code}"
        );
    }

    /// Self-referential named objects are emitted from the type-registry pass (nested helpers), not
    /// as the top-level `*Response` struct.
    #[test]
    fn self_referential_nested_type_uses_box() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        let recursive_row = TypeDef {
            name: "RecursiveRow".to_string(),
            kind: TypeKind::Object,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Named("child".to_string()),
                field_type: TypeDef {
                    name: "RecursiveRow".to_string(),
                    kind: TypeKind::Object,
                    fields: Some(vec![]),
                    protocol_type: Some("object".to_string()),
                    ..Default::default()
                },
                required: false,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            protocol_type: Some("object".to_string()),
            ..Default::default()
        };

        let wrapper = TypeDef {
            name: "object".to_string(),
            kind: TypeKind::Object,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Named("row".to_string()),
                field_type: recursive_row,
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            protocol_type: Some("object".to_string()),
            ..Default::default()
        };

        let method = RpcDef {
            name: "wrapper_demo".to_string(),
            result: Some(wrapper),
            ..Default::default()
        };

        let files = gen.generate(&[method]).expect("generate");
        let code =
            files.iter().find(|(name, _)| name == "responses.rs").expect("responses.rs").1.clone();

        assert!(
            code.contains("pub child: Option<Box<RecursiveRow>>"),
            "nested recursive IR object needs Box, got:\n{code}"
        );
    }

    #[test]
    fn nested_map_of_map_does_not_emit_recursive_type_alias() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        let leaf = TypeDef {
            name: "GetRawAddrManMapValue".to_string(),
            type_identity: Some("GetRawAddrManMapValue".to_string()),
            kind: TypeKind::Object,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Named("address".to_string()),
                field_type: TypeDef {
                    name: "string".to_string(),
                    kind: TypeKind::Primitive,
                    protocol_type: Some("string".to_string()),
                    ..Default::default()
                },
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            protocol_type: Some("object".to_string()),
            ..Default::default()
        };

        let inner_map = TypeDef {
            name: "GetRawAddrManMapValue".to_string(),
            type_identity: Some("GetRawAddrManMapValue".to_string()),
            kind: TypeKind::Map,
            protocol_type: Some("object-dynamic".to_string()),
            map_value: Some(Box::new(leaf)),
            map_key_protocol_type: Some("string".to_string()),
            ..Default::default()
        };

        let outer_map = TypeDef {
            name: "GetRawAddrManResultMap".to_string(),
            type_identity: Some("GetRawAddrManResultMap".to_string()),
            kind: TypeKind::Map,
            protocol_type: Some("object-dynamic".to_string()),
            map_value: Some(Box::new(inner_map)),
            map_key_protocol_type: Some("string".to_string()),
            ..Default::default()
        };

        let method = RpcDef {
            name: "getrawaddrman".to_string(),
            result: Some(outer_map),
            ..Default::default()
        };

        let files = gen.generate(&[method]).expect("generate");
        let code =
            files.iter().find(|(name, _)| name == "responses.rs").expect("responses.rs").1.clone();

        assert!(
            code.contains("pub struct GetRawAddrManMapValue"),
            "leaf object must be a struct, got:\n{code}"
        );
        assert!(code.contains("pub address:"), "leaf object must keep address field, got:\n{code}");
        assert!(
            !code.contains(
                "pub type GetRawAddrManMapValue = BTreeMap<String, GetRawAddrManMapValue>"
            ),
            "must not emit recursive map alias, got:\n{code}"
        );
        assert!(
            code.contains(
                "pub struct GetRawAddrManResponse(pub BTreeMap<String, BTreeMap<String, GetRawAddrManMapValue>>)"
            ),
            "response must be map of map of leaf object, got:\n{code}"
        );
    }

    #[test]
    fn emit_in_struct_false_skips_field() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());
        let result_ty = TypeDef {
            name: "object".to_string(),
            kind: TypeKind::Object,
            fields: Some(vec![
                ir::FieldDef {
                    key: ir::FieldKey::Named("keep_me".to_string()),
                    field_type: TypeDef {
                        name: "string".to_string(),
                        kind: TypeKind::Primitive,
                        protocol_type: Some("string".to_string()),
                        ..Default::default()
                    },
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                },
                ir::FieldDef {
                    key: ir::FieldKey::Named("tx_1".to_string()),
                    field_type: TypeDef {
                        name: "string".to_string(),
                        kind: TypeKind::Primitive,
                        protocol_type: Some("string".to_string()),
                        ..Default::default()
                    },
                    required: true,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: Some(false),
                    force_optional: None,
                },
            ]),
            protocol_type: Some("object".to_string()),
            ..Default::default()
        };
        let method = RpcDef {
            name: "emit_skip_demo".to_string(),
            result: Some(result_ty),
            ..Default::default()
        };
        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");
        assert!(code.contains("keep_me"), "got:\n{code}");
        assert!(!code.contains("tx_1"), "scaffold field should be omitted, got:\n{code}");
    }

    #[test]
    fn union_response_emits_vec_and_btreemap_variants() {
        let version = ProtocolVersion::from_str("30.0.0").unwrap();
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        let str_prim = TypeDef {
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
            ..Default::default()
        };

        let array_branch = TypeDef {
            name: "array".to_string(),
            description: String::new(),
            kind: TypeKind::Array,
            fields: Some(vec![ir::FieldDef {
                key: ir::FieldKey::Anonymous(0),
                field_type: str_prim,
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("array".to_string()),
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let num_prim = TypeDef {
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
            ..Default::default()
        };

        let map_branch = TypeDef {
            name: "map".to_string(),
            description: String::new(),
            kind: TypeKind::Map,
            fields: None,
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: None,
            canonical_name: None,
            condition: None,
            map_value: Some(Box::new(num_prim)),
            map_key_protocol_type: Some("hex".to_string()),
            ..Default::default()
        };

        let result_ty = TypeDef {
            name: "DemoExclusiveUnionResponse".to_string(),
            description: String::new(),
            kind: TypeKind::Union,
            fields: None,
            variants: None,
            union_variants: Some(vec![
                ir::UnionVariantDef {
                    name: "TxidStrings".to_string(),
                    description: String::new(),
                    condition: None,
                    type_def: array_branch,
                },
                ir::UnionVariantDef {
                    name: "TxidToNumber".to_string(),
                    description: String::new(),
                    condition: None,
                    type_def: map_branch,
                },
            ]),
            base_type: None,
            protocol_type: None,
            canonical_name: None,
            condition: None,
            ..Default::default()
        };

        let method = RpcDef {
            name: "demo_exclusive_union".to_string(),
            result: Some(result_ty),
            ..Default::default()
        };

        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");

        assert!(
            code.contains("#[serde(untagged)]"),
            "union response should use untagged enum, got:\n{code}"
        );
        assert!(code.contains("Vec<String>"), "expected Vec<String> array variant, got:\n{code}");
        assert!(
            code.contains("BTreeMap<bitcoin::Txid, u64>"),
            "expected BTreeMap txid map variant, got:\n{code}"
        );
    }

    #[test]
    fn top_level_map_result_generates_transparent_map_wrapper() {
        let version = ProtocolVersion::from_str("30.0.0").expect("version");
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

        let result_ty = TypeDef {
            name: "LoggingResultMap".to_string(),
            description: String::new(),
            kind: TypeKind::Map,
            fields: None,
            variants: None,
            union_variants: None,
            base_type: None,
            protocol_type: Some("object-dynamic".to_string()),
            canonical_name: None,
            condition: None,
            map_value: Some(Box::new(TypeDef {
                name: "boolean".to_string(),
                description: String::new(),
                kind: TypeKind::Primitive,
                fields: None,
                variants: None,
                union_variants: None,
                base_type: None,
                protocol_type: Some("boolean".to_string()),
                canonical_name: None,
                condition: None,
                ..Default::default()
            })),
            map_key_protocol_type: Some("string".to_string()),
            ..Default::default()
        };

        let method =
            RpcDef { name: "logging".to_string(), result: Some(result_ty), ..Default::default() };

        let code = gen
            .generate_method_response(&method)
            .expect("generation must succeed")
            .expect("response must be generated");

        assert!(
            code.contains("#[serde(transparent)]"),
            "expected transparent wrapper, got:\n{code}"
        );
        assert!(
            code.contains("pub struct LoggingResponse(pub BTreeMap<String, bool>);"),
            "expected map wrapper shape, got:\n{code}"
        );
    }

    #[test]
    fn generated_output_does_not_emit_resultmap_string_aliases() {
        let version = ProtocolVersion::from_str("30.0.0").expect("version");
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());
        let methods = vec![RpcDef {
            name: "logging".to_string(),
            description: String::new(),
            params: vec![],
            result: Some(TypeDef {
                name: "LoggingResultMap".to_string(),
                description: String::new(),
                kind: TypeKind::Map,
                fields: None,
                variants: None,
                union_variants: None,
                base_type: None,
                protocol_type: Some("object-dynamic".to_string()),
                canonical_name: None,
                condition: None,
                map_value: Some(Box::new(TypeDef {
                    name: "boolean".to_string(),
                    description: String::new(),
                    kind: TypeKind::Primitive,
                    fields: None,
                    variants: None,
                    union_variants: None,
                    base_type: None,
                    protocol_type: Some("boolean".to_string()),
                    canonical_name: None,
                    condition: None,
                    ..Default::default()
                })),
                map_key_protocol_type: Some("string".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }];
        let files = gen.generate(&methods).expect("generate");
        let responses = files
            .iter()
            .find(|(name, _)| name == "responses.rs")
            .map(|(_, content)| content)
            .expect("responses.rs");
        assert!(
            !responses.contains("pub type LoggingResultMap = String;"),
            "map aliases must never degrade to String:\n{responses}"
        );
    }

    #[test]
    fn analyzepsbt_emits_deep_nested_object_structs() {
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
        let ir_path = workspace_root.join("resources/ir/bitcoin.ir.json");
        let ir = ir::ProtocolIR::from_file(&ir_path)
            .unwrap_or_else(|e| panic!("load IR {}: {e}", ir_path.display()));
        let methods: Vec<RpcDef> = ir.get_rpc_methods().into_iter().cloned().collect();
        let version = ProtocolVersion::from_str("30.2.0").expect("protocol version");
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());
        let files = gen.generate(&methods).expect("generate responses");
        let responses = files
            .iter()
            .find(|(name, _)| name == "responses.rs")
            .map(|(_, content)| content)
            .expect("responses.rs");
        assert!(
            responses.contains("pub struct AnalyzePsbtMissing "),
            "expected AnalyzePsbtMissing definition; codegen may skip emitting nested types referenced by fields"
        );
    }

    #[test]
    fn fallback_inventory_regression_guard() {
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
        let ir_path = workspace_root.join("resources/ir/bitcoin.ir.json");
        let ir = ir::ProtocolIR::from_file(&ir_path)
            .unwrap_or_else(|e| panic!("load IR {}: {e}", ir_path.display()));
        let methods: Vec<RpcDef> = ir.get_rpc_methods().into_iter().cloned().collect();

        clear_fallback_events();
        let version = ProtocolVersion::from_str("30.0.0").expect("protocol version");
        let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());
        let _ = gen.generate(&methods).expect("generate responses");
        let events = fallback_events_snapshot();

        let allowed_reasons: std::collections::BTreeSet<&str> = std::collections::BTreeSet::from([
            "generic_object_without_named_shape",
            "unmapped_alias_default_string",
        ]);
        for event in &events {
            assert!(
                allowed_reasons.contains(event.reason.as_str()),
                "unexpected fallback reason `{}` in event {:?}; review schema/codegen precision",
                event.reason,
                event
            );
        }
        // Full `bitcoin.ir.json` legitimately triggers many `generic_object_without_named_shape`
        // events until OpenRPC gives stable names to every nested object; cap avoids silent growth.
        assert!(
            events.len() <= 128,
            "fallback event count increased unexpectedly ({} > 128); investigate codegen/IR regression",
            events.len()
        );
    }

    #[test]
    fn no_rpc_name_specific_codegen_conditionals() {
        let src = include_str!("version_specific_response_type.rs");
        let forbidden = [
            format!("if {} == ", "rpc_name"),
            format!("if {} == ", "method.name"),
            format!("{}::", "raw_response_policy"),
        ];
        for needle in forbidden {
            assert!(
                !src.contains(&needle),
                "version-specific codegen must remain IR-driven; found forbidden pattern: {needle}"
            );
        }
    }
}
