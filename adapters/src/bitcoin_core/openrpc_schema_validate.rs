// SPDX-License-Identifier: CC0-1.0

//! Runtime Draft 7 JSON Schema validation against Bitcoin Core OpenRPC dumps.
//!
//! Port of `corpus/openrpc-generator` `client` call-path checks: validate parameters before
//! send and results before decode. Ethos clients use positional JSON-RPC arrays, so the
//! registry ships both a positional tuple schema and a named-object schema per method.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

use serde_json::{Map, Value};

/// Direction of a schema check (parameters vs result).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaDirection {
    /// Request parameters.
    Parameters,
    /// Response result.
    Result,
}

impl SchemaDirection {
    fn as_str(self) -> &'static str {
        match self {
            Self::Parameters => "parameters",
            Self::Result => "result",
        }
    }
}

/// Schema validation failure with method context.
#[derive(Debug, Clone)]
pub struct SchemaValidationError {
    /// RPC method name.
    pub method: String,
    /// Whether params or result failed.
    pub direction: SchemaDirection,
    /// Human-readable reason.
    pub reason: String,
    /// JSON Schema instance path (`root` when empty).
    pub path: String,
}

impl fmt::Display for SchemaValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {} at {}", self.method, self.direction.as_str(), self.reason, self.path)
    }
}

impl std::error::Error for SchemaValidationError {}

/// Params + result schemas for one OpenRPC method.
#[derive(Debug, Clone)]
pub struct MethodWireSchemas {
    /// Draft 7 object schema for named-parameter calls (`additionalProperties: false`).
    pub named_params: Value,
    /// Draft 7 tuple-array schema for positional JSON-RPC `params`.
    pub positional_params: Value,
    /// Result schema from OpenRPC `result.schema`.
    pub result: Value,
}

/// Method name → wire schemas, built from an OpenRPC document.
#[derive(Debug, Clone, Default)]
pub struct WireSchemaRegistry {
    methods: BTreeMap<String, MethodWireSchemas>,
}

impl WireSchemaRegistry {
    /// Build a registry from a `getopenrpcinfo` / OpenRPC 1.4.1 document.
    pub fn from_openrpc_document(document: &Value) -> Result<Self, String> {
        let methods =
            document.get("methods").and_then(Value::as_array).ok_or("missing methods array")?;
        let mut registry = Self::default();
        for method in methods {
            let name =
                method.get("name").and_then(Value::as_str).ok_or("method is missing a name")?;
            let params = method
                .get("params")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{name} is missing params"))?;
            let (named_params, positional_params) = build_params_schemas(name, params)?;
            let result = method
                .get("result")
                .and_then(|result| result.get("schema"))
                .cloned()
                .ok_or_else(|| format!("{name} result is missing a schema"))?;
            // Strip non-Draft-7 extension keywords so jsonschema compile stays clean.
            let result = strip_extension_keywords(&result);
            registry.methods.insert(
                name.to_owned(),
                MethodWireSchemas { named_params, positional_params, result },
            );
        }
        Ok(registry)
    }

    /// Look up schemas for a method.
    pub fn get(&self, method: &str) -> Option<&MethodWireSchemas> { self.methods.get(method) }

    /// Iterate method entries in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &MethodWireSchemas)> {
        self.methods.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Validate positional JSON-RPC `params` against the method's tuple schema.
    ///
    /// Unknown methods succeed (no schema to enforce).
    pub fn validate_params_positional(
        &self,
        method: &str,
        params: &[Value],
    ) -> Result<(), SchemaValidationError> {
        let Some(schemas) = self.methods.get(method) else {
            return Ok(());
        };
        let instance = Value::Array(params.to_vec());
        validate_instance(
            method,
            SchemaDirection::Parameters,
            &schemas.positional_params,
            &instance,
        )
    }

    /// Validate a named-parameter object against the method's object schema.
    pub fn validate_params_named(
        &self,
        method: &str,
        params: &Value,
    ) -> Result<(), SchemaValidationError> {
        let Some(schemas) = self.methods.get(method) else {
            return Ok(());
        };
        validate_instance(method, SchemaDirection::Parameters, &schemas.named_params, params)
    }

    /// Validate an RPC result against the method's result schema.
    pub fn validate_result(
        &self,
        method: &str,
        result: &Value,
    ) -> Result<(), SchemaValidationError> {
        let Some(schemas) = self.methods.get(method) else {
            return Ok(());
        };
        validate_instance(method, SchemaDirection::Result, &schemas.result, result)
    }

    /// Emit a standalone Rust module for generated client crates (`schema-validate` feature).
    ///
    /// The module exposes `validate_params` / `validate_result` using positional arrays,
    /// matching Ethos `TransportExt::call`.
    pub fn emit_generated_module_source(&self) -> String {
        let mut source = String::from(
            r#"//! Generated OpenRPC wire schema registry. Do not edit.
//!
//! Enabled with feature `schema-validate`. Validates positional params before send and
//! results before serde decode (Draft 7), matching corpus/openrpc-generator client mode.

#![cfg(feature = "schema-validate")]

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

fn schema_error(method: &str, direction: &str, error: &jsonschema::error::ValidationError<'_>) -> String {
    let reason = match error.kind() {
        jsonschema::error::ValidationErrorKind::OneOfMultipleValid { .. } =>
            "matches multiple oneOf branches".to_owned(),
        jsonschema::error::ValidationErrorKind::OneOfNotValid { .. } =>
            "matches no oneOf branch".to_owned(),
        kind => format!("fails JSON Schema {}", kind.keyword()),
    };
    let path = error.instance_path().to_string();
    let path = if path.is_empty() { "root" } else { &path };
    format!("{method} {direction}: {reason} at {path}")
}

fn validate(method: &str, direction: &str, schema: &Value, instance: &Value) -> Result<(), String> {
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .build(schema)
        .map_err(|error| format!("{method} {direction} schema: {error}"))?;
    validator
        .validate(instance)
        .map_err(|error| schema_error(method, direction, &error))
}

struct MethodSchemas {
    positional_params: Value,
    result: Value,
}

fn registry() -> &'static HashMap<&'static str, MethodSchemas> {
    static REGISTRY: OnceLock<HashMap<&'static str, MethodSchemas>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut map = HashMap::new();
"#,
        );
        for (name, schemas) in &self.methods {
            let params_lit = format!("{:?}", schemas.positional_params.to_string());
            let result_lit = format!("{:?}", schemas.result.to_string());
            source.push_str(&format!(
                r#"        map.insert({name:?}, MethodSchemas {{
            positional_params: serde_json::from_str({params_lit}).expect("embedded params schema"),
            result: serde_json::from_str({result_lit}).expect("embedded result schema"),
        }});
"#
            ));
        }
        source.push_str(
            r#"        map
    })
}

/// Validate positional JSON-RPC params. Unknown methods are skipped.
pub fn validate_params(method: &str, params: &[Value]) -> Result<(), String> {
    let Some(schemas) = registry().get(method) else {
        return Ok(());
    };
    let instance = Value::Array(params.to_vec());
    validate(method, "parameters", &schemas.positional_params, &instance)
}

/// Validate an RPC result. Unknown methods are skipped.
pub fn validate_result(method: &str, result: &Value) -> Result<(), String> {
    let Some(schemas) = registry().get(method) else {
        return Ok(());
    };
    validate(method, "result", &schemas.result, result)
}
"#,
        );
        source
    }
}

fn build_params_schemas(method: &str, params: &[Value]) -> Result<(Value, Value), String> {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let mut items = Vec::new();
    for param in params {
        let param_name = param
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{method} parameter is missing a name"))?;
        let schema = param
            .get("schema")
            .cloned()
            .ok_or_else(|| format!("{method}.{param_name} is missing a schema"))?;
        let schema = strip_extension_keywords(&schema);
        if properties.insert(param_name.to_owned(), schema.clone()).is_some() {
            return Err(format!("{method} has duplicate parameter {param_name}"));
        }
        if param.get("required").and_then(Value::as_bool).unwrap_or(false) {
            required.push(Value::String(param_name.to_owned()));
        }
        items.push(schema);
    }
    let named = Value::Object({
        let mut map = Map::new();
        map.insert("type".into(), Value::String("object".into()));
        map.insert("properties".into(), Value::Object(properties));
        map.insert("required".into(), Value::Array(required));
        map.insert("additionalProperties".into(), Value::Bool(false));
        map
    });
    let positional = Value::Object({
        let mut map = Map::new();
        map.insert("type".into(), Value::String("array".into()));
        map.insert("items".into(), Value::Array(items));
        map.insert("additionalItems".into(), Value::Bool(false));
        map
    });
    Ok((named, positional))
}

/// Remove `x-bitcoin-*` keys so Draft 7 compilers ignore unknown keywords.
fn strip_extension_keywords(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, value) in map {
                if key.starts_with("x-bitcoin-") {
                    continue;
                }
                out.insert(key.clone(), strip_extension_keywords(value));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(strip_extension_keywords).collect()),
        other => other.clone(),
    }
}

fn validate_instance(
    method: &str,
    direction: SchemaDirection,
    schema: &Value,
    instance: &Value,
) -> Result<(), SchemaValidationError> {
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .build(schema)
        .map_err(|error| SchemaValidationError {
            method: method.to_owned(),
            direction,
            reason: format!("schema: {error}"),
            path: "root".into(),
        })?;
    match validator.validate(instance) {
        Ok(()) => Ok(()),
        Err(error) => Err(format_validation_error(method, direction, &error)),
    }
}

fn format_validation_error(
    method: &str,
    direction: SchemaDirection,
    error: &jsonschema::error::ValidationError<'_>,
) -> SchemaValidationError {
    let reason = match error.kind() {
        jsonschema::error::ValidationErrorKind::OneOfMultipleValid { .. } =>
            "matches multiple oneOf branches".to_owned(),
        jsonschema::error::ValidationErrorKind::OneOfNotValid { .. } =>
            "matches no oneOf branch".to_owned(),
        kind => format!("fails JSON Schema {}", kind.keyword()),
    };
    let path = error.instance_path().to_string();
    let path = if path.is_empty() { "root".to_owned() } else { path };
    SchemaValidationError { method: method.to_owned(), direction, reason, path }
}

/// Process-global registry loaded once from an OpenRPC document (optional helper).
pub fn global_registry() -> &'static OnceLock<WireSchemaRegistry> {
    static REGISTRY: OnceLock<WireSchemaRegistry> = OnceLock::new();
    &REGISTRY
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sample_doc() -> Value {
        json!({
            "openrpc": "1.4.1",
            "info": {"title": "t", "version": "0"},
            "methods": [{
                "name": "echo",
                "params": [
                    {"name": "msg", "required": true, "schema": {"type": "string"}},
                    {"name": "n", "required": false, "schema": {"type": "integer", "x-bitcoin-unit": "unix-time"}}
                ],
                "result": {"name": "result", "schema": {"type": "string"}}
            }]
        })
    }

    #[test]
    fn positional_params_validate() {
        let reg = WireSchemaRegistry::from_openrpc_document(&sample_doc()).expect("registry");
        reg.validate_params_positional("echo", &[json!("hi")]).expect("ok");
        let err = reg.validate_params_positional("echo", &[json!(1)]).expect_err("type mismatch");
        assert!(err.to_string().contains("parameters"), "{err}");
    }

    #[test]
    fn named_params_validate() {
        let reg = WireSchemaRegistry::from_openrpc_document(&sample_doc()).expect("registry");
        reg.validate_params_named("echo", &json!({"msg": "hi"})).expect("ok");
        let err = reg
            .validate_params_named("echo", &json!({"msg": "hi", "extra": true}))
            .expect_err("additional");
        assert!(err.to_string().contains("parameters"), "{err}");
    }

    #[test]
    fn result_one_of_multiple_valid_is_explicit() {
        let doc = json!({
            "methods": [{
                "name": "overlap",
                "params": [],
                "result": {"name": "r", "schema": {
                    "oneOf": [
                        {"type": "object", "properties": {"a": {"type": "integer"}}, "additionalProperties": false},
                        {"type": "object", "properties": {"a": {"type": "integer"}}, "additionalProperties": false}
                    ]
                }}
            }]
        });
        let reg = WireSchemaRegistry::from_openrpc_document(&doc).expect("registry");
        let err = reg.validate_result("overlap", &json!({"a": 1})).expect_err("overlap");
        assert!(err.reason.contains("multiple oneOf"), "{err:?}");
    }

    #[test]
    fn emit_module_contains_method() {
        let reg = WireSchemaRegistry::from_openrpc_document(&sample_doc()).expect("registry");
        let src = reg.emit_generated_module_source();
        assert!(src.contains("\"echo\""));
        assert!(src.contains("validate_params"));
        assert!(src.contains("schema-validate"));
    }
}
