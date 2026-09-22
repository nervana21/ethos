// SPDX-License-Identifier: CC0-1.0

//! Strict OpenRPC schema keyword allowlist audit (Draft 7).
//!
//! Port of `corpus/openrpc-generator` `audit` mode: every schema node must use a known
//! keyword set, must compile as Draft 7 JSON Schema, and `default` values that fail their
//! own schema are reported as warnings.

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use serde_json::{Map, Value};

/// Aggregate audit findings for one OpenRPC document.
#[derive(Debug, Default)]
pub struct AuditReport {
    /// Number of methods walked.
    pub method_count: usize,
    /// Number of parameters walked.
    pub parameter_count: usize,
    /// Number of schema object nodes walked.
    pub schema_count: usize,
    /// Keyword occurrence counts across all schema nodes.
    pub keyword_counts: BTreeMap<String, usize>,
    /// Soft findings (e.g. `default` fails its schema).
    pub warnings: Vec<String>,
}

impl fmt::Display for AuditReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Audited {} methods, {} parameters, {} schema nodes.",
            self.method_count, self.parameter_count, self.schema_count
        )?;
        for (keyword, count) in &self.keyword_counts {
            writeln!(f, "  {keyword}: {count}")?;
        }
        for warning in &self.warnings {
            writeln!(f, "Warning: {warning}")?;
        }
        Ok(())
    }
}

/// Check every method schema before strict type or client generation.
pub fn inspect(document: &Value) -> Result<AuditReport, String> {
    let root = object(document, "document")?;
    check_keys(root, &["openrpc", "info", "methods"], "document")?;
    if root.get("openrpc").and_then(Value::as_str) != Some("1.4.1") {
        return Err("document.openrpc must be 1.4.1".into());
    }
    let info = object(root.get("info").ok_or("document.info is missing")?, "document.info")?;
    check_keys(info, &["title", "version", "description"], "document.info")?;
    string_field(info, "title", "document.info")?;
    string_field(info, "version", "document.info")?;
    let methods = document
        .get("methods")
        .and_then(Value::as_array)
        .ok_or("document.methods must be an array")?;
    let mut report = AuditReport::default();
    let mut names = HashSet::new();
    for (method_index, method) in methods.iter().enumerate() {
        let path = format!("methods[{method_index}]");
        let method_object = object(method, &path)?;
        check_keys(
            method_object,
            &["name", "description", "params", "result", "x-bitcoin-category"],
            &path,
        )?;
        let name = string_field(method_object, "name", &path)?;
        if !names.insert(name) {
            return Err(format!("{path}.name: duplicate method {name:?}"));
        }
        let params = method_object
            .get("params")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{path}.params must be an array"))?;
        let mut param_names = HashSet::new();
        for (param_index, param) in params.iter().enumerate() {
            let param_path = format!("{path}.params[{param_index}]");
            let param = object(param, &param_path)?;
            check_keys(
                param,
                &[
                    "name",
                    "required",
                    "schema",
                    "description",
                    "x-bitcoin-aliases",
                    "x-bitcoin-placeholder",
                    "x-bitcoin-also-positional",
                ],
                &param_path,
            )?;
            let param_name = string_field(param, "name", &param_path)?;
            if !param_names.insert(param_name) {
                return Err(format!("{param_path}.name: duplicate parameter {param_name:?}"));
            }
            if !param.get("required").is_some_and(Value::is_boolean) {
                return Err(format!("{param_path}.required must be a boolean"));
            }
            if param.get("x-bitcoin-placeholder").is_some_and(|value| !value.is_boolean())
                || param.get("x-bitcoin-also-positional").is_some_and(|value| !value.is_boolean())
            {
                return Err(format!("{param_path}: Bitcoin flags must be booleans"));
            }
            if let Some(aliases) = param.get("x-bitcoin-aliases") {
                if !aliases.as_array().is_some_and(|values| values.iter().all(Value::is_string)) {
                    return Err(format!("{param_path}.x-bitcoin-aliases must be strings"));
                }
            }
            let schema =
                param.get("schema").ok_or_else(|| format!("{param_path}.schema is missing"))?;
            inspect_schema(schema, &format!("{param_path}.schema"), &mut report)?;
            compile_schema(schema, &format!("{param_path}.schema"))?;
            report.parameter_count += 1;
        }
        let result_path = format!("{path}.result");
        let result = object(
            method_object.get("result").ok_or_else(|| format!("{result_path} is missing"))?,
            &result_path,
        )?;
        check_keys(result, &["name", "schema"], &result_path)?;
        string_field(result, "name", &result_path)?;
        let schema =
            result.get("schema").ok_or_else(|| format!("{result_path}.schema is missing"))?;
        inspect_schema(schema, &format!("{result_path}.schema"), &mut report)?;
        compile_schema(schema, &format!("{result_path}.schema"))?;
        report.method_count += 1;
    }
    Ok(report)
}

fn check_keys(object: &Map<String, Value>, allowed: &[&str], path: &str) -> Result<(), String> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("{path}.{key}: unsupported field"));
        }
    }
    Ok(())
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, String> {
    value.as_object().ok_or_else(|| format!("{path} must be an object"))
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{path}.{key} must be a nonempty string"))
}

fn inspect_schema(schema: &Value, path: &str, report: &mut AuditReport) -> Result<(), String> {
    let schema_value = schema;
    let schema = object(schema, path)?;
    report.schema_count += 1;
    for (key, value) in schema {
        *report.keyword_counts.entry(key.clone()).or_default() += 1;
        let here = format!("{path}.{key}");
        match key.as_str() {
            "type" => {
                if !matches!(
                    value.as_str(),
                    Some("null" | "boolean" | "object" | "array" | "number" | "integer" | "string")
                ) {
                    return Err(format!("{here}: unsupported or invalid type {value}"));
                }
            }
            "properties" => {
                let properties = object(value, &here)?;
                for (property_name, property_schema) in properties {
                    inspect_schema(property_schema, &format!("{here}[{property_name:?}]"), report)?;
                }
            }
            "required" => {
                let values = value
                    .as_array()
                    .ok_or_else(|| format!("{here} must be an array of unique strings"))?;
                let mut names = HashSet::new();
                for item in values {
                    let Some(name) = item.as_str() else {
                        return Err(format!("{here} must be an array of unique strings"));
                    };
                    if !names.insert(name) {
                        return Err(format!("{here}: duplicate property {name:?}"));
                    }
                }
            }
            "items" =>
                if let Some(items) = value.as_array() {
                    if items.is_empty() {
                        return Err(format!("{here} must have at least one tuple item"));
                    }
                    for (index, item) in items.iter().enumerate() {
                        inspect_schema(item, &format!("{here}[{index}]"), report)?;
                    }
                } else {
                    inspect_schema(value, &here, report)?;
                },
            "additionalProperties" | "additionalItems" =>
                if !value.is_boolean() {
                    inspect_schema(value, &here, report)?;
                },
            "oneOf" | "anyOf" => {
                let variants = value
                    .as_array()
                    .filter(|variants| !variants.is_empty())
                    .ok_or_else(|| format!("{here} must be a nonempty array"))?;
                for (index, variant) in variants.iter().enumerate() {
                    inspect_schema(variant, &format!("{here}[{index}]"), report)?;
                }
            }
            "minItems" | "maxItems" =>
                if value.as_u64().is_none() {
                    return Err(format!("{here} must be a nonnegative integer"));
                },
            "pattern" | "description" =>
                if !value.is_string() {
                    return Err(format!("{here} must be a string"));
                },
            "const" | "default" => {}
            "x-bitcoin-unit" =>
                if !matches!(value.as_str(), Some("amount" | "unix-time")) {
                    return Err(format!("{here}: unsupported unit {value}"));
                },
            "x-bitcoin-default-hint" | "x-bitcoin-type-override" =>
                if !value.is_string() {
                    return Err(format!("{here} must be a string"));
                },
            "x-bitcoin-also-positional" | "x-bitcoin-placeholder" =>
                if !value.is_boolean() {
                    return Err(format!("{here} must be a boolean"));
                },
            // Forward-compatible with Bitcoin Core PR #36175.
            "x-bitcoin-discriminated-result" =>
                if !value.is_object() {
                    return Err(format!("{here} must be an object"));
                },
            _ => return Err(format!("{here}: unsupported schema keyword")),
        }
    }
    if (schema.contains_key("properties")
        || schema.contains_key("required")
        || schema.contains_key("additionalProperties"))
        && schema.get("type").and_then(Value::as_str) != Some("object")
    {
        return Err(format!("{path}: object keywords require type object"));
    }
    if (schema.contains_key("items")
        || schema.contains_key("additionalItems")
        || schema.contains_key("minItems")
        || schema.contains_key("maxItems"))
        && schema.get("type").and_then(Value::as_str) != Some("array")
    {
        return Err(format!("{path}: array keywords require type array"));
    }
    if schema.contains_key("additionalItems") && !schema.get("items").is_some_and(Value::is_array) {
        return Err(format!("{path}.additionalItems requires tuple items"));
    }
    if let (Some(min), Some(max)) = (schema.get("minItems"), schema.get("maxItems")) {
        if min.as_u64() > max.as_u64() {
            return Err(format!("{path}: minItems exceeds maxItems"));
        }
    }
    if let Some(default) = schema.get("default") {
        let validator = compile_schema(schema_value, path)?;
        if !validator.is_valid(default) {
            report.warnings.push(format!("{path}.default does not satisfy its schema"));
        }
    }
    Ok(())
}

fn compile_schema(schema: &Value, path: &str) -> Result<jsonschema::Validator, String> {
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .build(schema)
        .map_err(|error| format!("{path}: invalid JSON Schema: {error}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn with_header(mut document: Value) -> Value {
        document["openrpc"] = json!("1.4.1");
        document["info"] = json!({"title": "test", "version": "0"});
        document
    }

    #[test]
    fn malformed_pattern_fails() {
        let document = with_header(json!({"methods": [{
            "name": "foo", "params": [],
            "result": {"name": "result", "schema": {"type": "string", "pattern": "["}}
        }]}));
        let error = inspect(&document).unwrap_err();
        assert!(error.contains("invalid JSON Schema"), "{error}");
    }

    #[test]
    fn invalid_default_is_a_warning() {
        let document = with_header(json!({"methods": [{
            "name": "foo", "params": [{"name": "p", "required": false,
                "schema": {"type": "string", "pattern": "^a+$", "default": "b"}}],
            "result": {"name": "result", "schema": {"type": "null"}}
        }]}));
        let report = inspect(&document).expect("audit");
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("default does not satisfy"));
    }

    #[test]
    fn unknown_schema_keyword_fails_at_its_path() {
        let document = with_header(json!({"methods": [{
            "name": "foo", "params": [{"name": "p", "required": true,
                "schema": {"type": "object", "properties": {"arbitrary-key": {"type": "string", "mystery": true}}}}],
            "result": {"name": "result", "schema": {"type": "null"}}
        }]}));
        let error = inspect(&document).unwrap_err();
        assert!(error.contains("properties[\"arbitrary-key\"].mystery"), "{error}");
    }
}
