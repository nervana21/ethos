//! Version-specific response type generator
//!
//! This module enhances the response type generator to use version-specific
//! type metadata extracted from corepc to generate accurate types for each
//! Bitcoin Core version.

use std::fmt::Write as _;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};

use ir::{ProtocolIR, RpcDef};
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
        // Simplified - generate directly from IR result types instead of metadata
        let mut nested_types = std::collections::HashSet::new();
        for method in methods {
            if let Some(result) = &method.result {
                if let Some(fields) = &result.fields {
                    for field in fields {
                        self.collect_nested_types(&field.field_type.name, &mut nested_types);
                    }
                }
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
                if let Some(fields) = &result.fields {
                    for field in fields {
                        let field_type = &field.field_type.name;
                        if field_type.contains("BTreeMap") {
                            needs_btreemap = true;
                        }
                        if field_type.contains("HashMap") {
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
                        let rust_type = self.map_ir_type_to_rust(&field.field_type, &field.name);
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

        // Generate nested types first, recursively collecting more nested types
        let mut all_nested_types = nested_types.clone();
        let mut processed_types = std::collections::HashSet::new();

        while !all_nested_types.is_empty() {
            let current_types: Vec<String> = all_nested_types.drain().collect();

            for nested_type in &current_types {
                if processed_types.contains(nested_type) {
                    continue;
                }

                println!("[NESTED_TYPES] Generating type: {}", nested_type);
                if let Some(nested_struct) = self.generate_nested_type(nested_type)? {
                    println!("[NESTED_TYPES] Generated struct for: {}", nested_type);
                    out.push_str(&nested_struct);
                    out.push('\n');
                    processed_types.insert(nested_type.clone());

                    // Collect nested types - simplified since IR doesn't track per-type versions
                    // Nested types will be discovered when generating from IR result types
                } else {
                    println!("[NESTED_TYPES] Skipped generation for: {}", nested_type);
                    processed_types.insert(nested_type.clone());
                }
            }
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

    /// Fields that bitcoind may omit; force Option<T> for these (rpc_name, field_name)
    fn optional_field_override(rpc_name: &str, field_name: &str) -> bool {
        matches!((rpc_name, field_name), ("analyzepsbt", "fee") | ("decodepsbt", "fee"))
    }

    /// Generate response type for a specific method
    fn generate_method_response(&self, rpc: &RpcDef) -> Result<Option<String>> {
        // Simplified - always generate from IR data since we removed metadata registries
        if let Some(result) = &rpc.result {
            if let Some(fields) = &result.fields {
                if !fields.is_empty() {
                    return Ok(Some(self.generate_from_ir_data(rpc, result)?));
                }
            }
        }

        // For methods without result types, generate unit structs
        self.generate_unit_response(rpc)
    }

    /// Generate response type from IR data (TypeDef.fields)
    fn generate_from_ir_data(&self, rpc: &RpcDef, result: &ir::TypeDef) -> Result<String> {
        let struct_name = self.response_struct_name(rpc);
        let mut buf = String::new();

        // Generate struct documentation using PascalCase canonical method name
        let canonical_name =
            crate::utils::canonical_from_adapter_method(&self.implementation, &rpc.name)
                .unwrap_or_else(|_| struct_name.replace("Response", ""));
        write_doc_line(&mut buf, &format!("Response for the `{}` RPC method", canonical_name), "")?;
        writeln!(&mut buf, "///")?;
        if !result.description.is_empty() {
            write_doc_comment(&mut buf, &result.description, "")?;
        }

        // Check if this method has conditional results (can return either string or object)
        // This happens when we have both simple type results and object results
        let has_conditional_results = self.check_conditional_results(rpc, result);

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

        // Generate fields from IR data
        if let Some(fields) = &result.fields {
            for field in fields {
                self.generate_ir_field(&mut buf, field, &struct_name, rpc.name.as_str())?;
            }
        }

        writeln!(&mut buf, "}}")?;

        // Generate custom Deserialize implementation if needed
        if has_conditional_results {
            self.generate_conditional_deserialize_impl(&mut buf, rpc, result, &struct_name)?;
        }

        Ok(buf)
    }

    /// Generate a field from IR FieldDef
    fn generate_ir_field(
        &self,
        buf: &mut String,
        field: &ir::FieldDef,
        _struct_name: &str,
        rpc_name: &str,
    ) -> Result<()> {
        // Generate field documentation
        if !field.description.is_empty() {
            write_doc_comment(buf, &field.description, "    ")?;
        }

        // Generate field definition
        let base_field_type = self.map_ir_type_to_rust(&field.field_type, &field.name);
        let field_name = self.sanitize_identifier(&field.name);
        let mut field_type = if field.required {
            base_field_type.clone()
        } else {
            format!("Option<{}>", base_field_type)
        };
        // Override: some Core fields are absent on certain networks/versions
        if field.name == "blockmintxfee"
            || field.name == "maxdatacarriersize"
            || field.name == "permitbaremultisig"
            || field.name == "limitclustercount"
            || field.name == "limitclustersize"
        {
            field_type =
                format!("Option<{}>", self.map_ir_type_to_rust(&field.field_type, &field.name));
        }
        // Override: bitcoind omits these fields in some modes/versions
        if Self::optional_field_override(rpc_name, &field.name) {
            field_type =
                format!("Option<{}>", self.map_ir_type_to_rust(&field.field_type, &field.name));
        }

        // Add serde rename attribute if the field name was changed
        if field_name != field.name {
            writeln!(buf, "    #[serde(rename = \"{}\")]", field.name)?;
        }

        // When we force Option for omitted fields, allow missing key to deserialize as None
        if Self::optional_field_override(rpc_name, &field.name) {
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
                // Use the adapter to map the type - this leverages the comprehensive BitcoinCoreTypeRegistry
                let method_result = types::MethodResult {
                    type_: type_def.name.clone(),
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
            ir::TypeKind::Object => {
                // For structured types, use the type name, but handle special cases
                if type_def.name == "object" || type_def.name == "array" {
                    "serde_json::Value".to_string()
                } else {
                    type_def.name.clone()
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
    fn generate_unit_response(&self, rpc: &RpcDef) -> Result<Option<String>> {
        let struct_name = self.response_struct_name(rpc);
        let mut buf = String::new();

        // Check if the method has a return type defined in the IR
        if let Some(result) = &rpc.result {
            match &result.kind {
                ir::TypeKind::Array => {
                    // Generate array wrapper for methods that return arrays
                    return Ok(Some(self.generate_array_wrapper(rpc, &struct_name)?));
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
            crate::utils::canonical_from_adapter_method(&self.implementation, &rpc.name)
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
    fn check_conditional_results(&self, _rpc: &RpcDef, result: &ir::TypeDef) -> bool {
        // Check if all fields are optional (indicating conditional results)
        if let Some(fields) = &result.fields {
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
        _rpc: &RpcDef,
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
        // Handle string case (when verbose=false)
        // Find the txid field (or first field) to populate from string
        let txid_field = if let Some(fields) = &result.fields {
            fields.iter().find(|f| f.name == "txid" || f.name.contains("txid"))
        } else {
            None
        };
        let param_name = if txid_field.is_some() { "v" } else { "_v" };
        writeln!(
            buf,
            "            fn visit_str<E>(self, {}: &str) -> Result<Self::Value, E>",
            param_name
        )?;
        writeln!(buf, "            where")?;
        writeln!(buf, "                E: de::Error,")?;
        writeln!(buf, "            {{")?;
        if let Some(fields) = &result.fields {
            if let Some(field) = txid_field {
                let field_name = self.sanitize_identifier(&field.name);
                let field_type = self.map_ir_type_to_rust(&field.field_type, &field.name);
                writeln!(
                    buf,
                    "                let {} = {}::from_str({}).map_err(de::Error::custom)?;",
                    field_name, field_type, param_name
                )?;
                writeln!(buf, "                Ok({} {{", struct_name)?;
                for f in fields {
                    let fn_name = self.sanitize_identifier(&f.name);
                    if f.name == field.name {
                        writeln!(buf, "                    {}: Some({}),", fn_name, fn_name)?;
                    } else {
                        writeln!(buf, "                    {}: None,", fn_name)?;
                    }
                }
                writeln!(buf, "                }})")?;
            } else {
                // Fallback: create struct with all fields as None
                writeln!(buf, "                Ok({} {{", struct_name)?;
                for f in fields {
                    let fn_name = self.sanitize_identifier(&f.name);
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
        if let Some(fields) = &result.fields {
            for f in fields {
                let fn_name = self.sanitize_identifier(&f.name);
                writeln!(buf, "                let mut {} = None;", fn_name)?;
            }
            writeln!(buf, "                while let Some(key) = map.next_key::<String>()? {{")?;
            for f in fields {
                let fn_name = self.sanitize_identifier(&f.name);
                writeln!(buf, "                    if key == \"{}\" {{", f.name)?;
                writeln!(buf, "                        if {}.is_some() {{", fn_name)?;
                writeln!(
                    buf,
                    "                            return Err(de::Error::duplicate_field(\"{}\"));",
                    f.name
                )?;
                writeln!(buf, "                        }}")?;
                let field_type = self.map_ir_type_to_rust(&f.field_type, &f.name);
                let base_field_type = self.map_ir_type_to_rust(&f.field_type, &f.name);
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
            for f in fields {
                let fn_name = self.sanitize_identifier(&f.name);
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
    fn response_struct_name(&self, rpc: &RpcDef) -> String {
        let canonical =
            crate::utils::canonical_from_adapter_method(&self.implementation, &rpc.name);
        match canonical {
            Ok(name) => format!("{}Response", name),
            Err(_) => {
                let snake =
                    crate::utils::protocol_rpc_method_to_rust_name(&self.implementation, &rpc.name)
                        .unwrap_or_else(|_| crate::utils::rpc_method_to_rust_name(&rpc.name));
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
        writeln!(&mut buf, "    /// Wrapped primitive value")?;
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

    /// Generate array wrapper for methods that return arrays
    fn generate_array_wrapper(&self, rpc: &RpcDef, struct_name: &str) -> Result<String> {
        let mut buf = String::new();

        // Generate struct documentation using PascalCase canonical method name
        let canonical_name =
            crate::utils::canonical_from_adapter_method(&self.implementation, &rpc.name)
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
        writeln!(&mut buf, "    /// Wrapped array value")?;
        writeln!(&mut buf, "    pub value: Vec<serde_json::Value>,")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        // Generate custom Deserialize implementation
        writeln!(&mut buf, "impl<'de> serde::Deserialize<'de> for {} {{", struct_name)?;
        writeln!(&mut buf, "    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>")?;
        writeln!(&mut buf, "    where")?;
        writeln!(&mut buf, "        D: serde::Deserializer<'de>,")?;
        writeln!(&mut buf, "    {{")?;
        writeln!(
            &mut buf,
            "        let value = Vec::<serde_json::Value>::deserialize(deserializer)?;"
        )?;
        writeln!(&mut buf, "        Ok(Self {{ value }})")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        // Generate From implementations for transparent conversion
        writeln!(&mut buf, "impl From<Vec<serde_json::Value>> for {} {{", struct_name)?;
        writeln!(&mut buf, "    fn from(value: Vec<serde_json::Value>) -> Self {{")?;
        writeln!(&mut buf, "        Self {{ value }}")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;
        writeln!(&mut buf)?;

        writeln!(&mut buf, "impl From<{}> for Vec<serde_json::Value> {{", struct_name)?;
        writeln!(&mut buf, "    fn from(wrapper: {}) -> Self {{", struct_name)?;
        writeln!(&mut buf, "        wrapper.value")?;
        writeln!(&mut buf, "    }}")?;
        writeln!(&mut buf, "}}")?;

        Ok(buf)
    }

    /// Collect nested types from a field type string
    fn collect_nested_types(
        &self,
        type_name: &str,
        nested_types: &mut std::collections::HashSet<String>,
    ) {
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

    /// Generate a placeholder struct for an undefined nested type
    fn generate_nested_type(&self, type_name: &str) -> Result<Option<String>> {
        // Simplified - generate type alias since IR doesn't track per-type versions
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
        writeln!(output, "/// Type alias for {}", type_name)?;
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
