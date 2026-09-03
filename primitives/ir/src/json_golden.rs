//! Match JSON sample payloads against IR [`TypeDef`] trees for conformance tests.
//!
//! Rules: unknown JSON object keys are allowed. IR fields with
//! `emit_in_struct == Some(false)` are skipped (not expected on the wire).

use serde_json::Value;

use crate::{FieldDef, TypeDef, TypeKind};

/// Returns `Ok(())` when `value` is compatible with `ty` for required wire fields.
pub fn validate_json_matches_type(ty: &TypeDef, value: &Value) -> Result<(), String> {
    validate_json_matches_type_at(ty, value, "")
}

fn validate_json_matches_type_at(ty: &TypeDef, value: &Value, path: &str) -> Result<(), String> {
    // OpenRPC often encodes a JSON array as `kind: Object` with `protocol_type: "array"` and one
    // inner field (`field`, `field_0`, …) for the element prototype. On the wire it is still a
    // JSON array, so validate like an array when we see that shape.
    if ty.kind == TypeKind::Object
        && ty.protocol_type.as_deref() == Some("array")
        && value.as_array().is_some()
    {
        if let Some(elem_ty) = object_wrapped_array_element_type(ty) {
            let arr = value.as_array().expect("checked is_array");
            if let Some(first) = arr.first() {
                validate_json_matches_type_at(elem_ty, first, &format!("{path}[0]"))?;
            }
            return Ok(());
        }
    }

    match ty.kind {
        TypeKind::Object => validate_object(ty, value, path),
        TypeKind::Array => validate_array(ty, value, path),
        TypeKind::Map => validate_map(ty, value, path),
        TypeKind::Optional => {
            if value.is_null() {
                return Ok(());
            }
            let inner =
                ty.fields.as_ref().and_then(|f| f.first()).map(|fd| &fd.field_type).ok_or_else(
                    || format!("{path}: optional TypeDef must carry inner type in fields[0]"),
                )?;
            validate_json_matches_type_at(inner, value, path)
        }
        TypeKind::Union => {
            let variants = ty
                .union_variants
                .as_ref()
                .ok_or_else(|| format!("{path}: Union TypeDef missing union_variants"))?;
            let mut errs = Vec::new();
            for v in variants {
                match validate_json_matches_type_at(&v.type_def, value, path) {
                    Ok(()) => return Ok(()),
                    Err(e) => errs.push(e),
                }
            }
            Err(format!(
                "{path}: JSON matched no union variant ({}): {}",
                variants.len(),
                errs.join(" | ")
            ))
        }
        TypeKind::Primitive | TypeKind::Enum | TypeKind::Alias | TypeKind::Custom =>
            validate_primitive_loose(ty, value, path),
    }
}

/// Element type for `Object` + `protocol_type: "array"` wrappers (see `validate_json_matches_type_at`).
fn object_wrapped_array_element_type(ty: &TypeDef) -> Option<&TypeDef> {
    let fields = ty.fields.as_ref()?;
    fields
        .iter()
        .find(|f| f.key.is_positional_zero() || f.key.as_ident() == "field_0")
        .map(|f| &f.field_type)
        .or_else(|| fields.first().map(|f| &f.field_type))
}

fn validate_primitive_loose(ty: &TypeDef, value: &Value, path: &str) -> Result<(), String> {
    let p = ty.protocol_type.as_deref().unwrap_or(ty.name.as_str());
    match p {
        "string" | "hex" if !value.is_string() =>
            Err(format!("{path}: expected JSON string for protocol_type={p}, got {value}")),
        "number" | "amount" if !value.is_number() =>
            Err(format!("{path}: expected JSON number for protocol_type={p}, got {value}")),
        "boolean" if !value.is_boolean() =>
            Err(format!("{path}: expected JSON bool for protocol_type={p}, got {value}")),
        "none" | "null" if !value.is_null() =>
            Err(format!("{path}: expected JSON null for protocol_type={p}, got {value}")),
        _ => Ok(()),
    }
}

fn validate_object(ty: &TypeDef, value: &Value, path: &str) -> Result<(), String> {
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{path}: expected JSON object for IR Object, got {value}"))?;
    let fields = ty.fields.as_ref().ok_or_else(|| format!("{path}: IR Object missing fields"))?;

    // OpenRPC array rows are sometimes modeled as an object whose only child is synthetic
    // `field_0` pointing at the real row type. JSON array elements are the row object itself
    // and do not contain a `field_0` key.
    if fields.len() == 1 {
        let f = &fields[0];
        if f.key.as_ident() == "field_0"
            && f.emit_in_struct != Some(false)
            && !obj.contains_key("field_0")
        {
            return validate_json_matches_type_at(&f.field_type, value, path);
        }
    }

    for field in fields {
        if field.emit_in_struct == Some(false) {
            continue;
        }
        if field.field_type.protocol_type.as_deref() == Some("elision") {
            continue;
        }
        if !field_wire_required(field) {
            continue;
        }
        let key = field
            .key
            .json_key()
            .ok_or_else(|| format!("{path}: required field has no JSON key (anonymous)"))?;
        let child_path = format!("{path}.{key}");
        let child = obj
            .get(key)
            .ok_or_else(|| format!("{child_path}: missing required key in JSON sample"))?;
        validate_json_matches_type_at(&field.field_type, child, &child_path)?;
    }
    Ok(())
}

fn validate_array(ty: &TypeDef, value: &Value, path: &str) -> Result<(), String> {
    let arr = value
        .as_array()
        .ok_or_else(|| format!("{path}: expected JSON array for IR Array, got {value}"))?;
    if let Some(elem_ty) = ty.array_element_type() {
        if let Some(first) = arr.first() {
            validate_json_matches_type_at(elem_ty, first, &format!("{path}[0]"))?;
        }
    }
    Ok(())
}

fn validate_map(ty: &TypeDef, value: &Value, path: &str) -> Result<(), String> {
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{path}: expected JSON object for IR Map, got {value}"))?;
    let val_ty = ty.map_value_type().ok_or_else(|| format!("{path}: IR Map missing map_value"))?;
    if let Some((_k, v)) = obj.iter().next() {
        validate_json_matches_type_at(val_ty, v, &format!("{path}.<value>"))?;
    }
    Ok(())
}

fn field_wire_required(field: &FieldDef) -> bool {
    field.required && field.force_optional != Some(true)
}
