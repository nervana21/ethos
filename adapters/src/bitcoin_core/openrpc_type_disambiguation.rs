// SPDX-License-Identifier: CC0-1.0

//! Split overloaded OpenRPC object names into distinct IR `TypeDef::name` / `type_identity` values.
//!
//! Core's OpenRPC often assigns one component name to structurally different JSON objects.
//! The IR type registry and Rust codegen assume one shape per name; without this pass,
//! nested helpers collapse incorrectly (e.g. every `decodepsbt` sub-object named `DecodePsbtRow`).
//!
//! Rule tables live in `resources/adapters/bitcoin_core_openrpc_disambiguation.json`.

use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use ir::{ProtocolDef, ProtocolIR, TypeDef, TypeKind};
use serde::Deserialize;

const DISAMBIGUATION_JSON: &str =
    include_str!("../../../resources/adapters/bitcoin_core_openrpc_disambiguation.json");

#[derive(Debug, Deserialize)]
struct DisambiguationFile {
    #[allow(dead_code)]
    version: u32,
    rpcs: HashMap<String, RpcRules>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RpcRules {
    row_source_name: Option<String>,
    sorted_field_equals: Vec<SortedFieldRule>,
    key_set_rules: Vec<KeySetRule>,
    field_object_renames: Vec<FieldObjectRename>,
}

#[derive(Debug, Deserialize)]
struct SortedFieldRule {
    fields: Vec<String>,
    type_identity: String,
}

#[derive(Debug, Deserialize)]
struct KeySetRule {
    kind: String,
    fields: Vec<String>,
    #[serde(default)]
    unless_any_of: Vec<String>,
    type_identity: String,
}

#[derive(Debug, Deserialize)]
struct FieldObjectRename {
    parent_field: String,
    when_object_name: String,
    type_identity: String,
}

fn rules_table() -> &'static DisambiguationFile {
    static RULES: OnceLock<DisambiguationFile> = OnceLock::new();
    RULES.get_or_init(|| {
        serde_json::from_str(DISAMBIGUATION_JSON).unwrap_or_else(|e| {
            panic!("bitcoin_core_openrpc_disambiguation.json: {e}");
        })
    })
}

fn decode_psbt_type_identity(keys: &[String], rules: &RpcRules) -> String {
    for rule in &rules.sorted_field_equals {
        if keys.len() == rule.fields.len() && keys.iter().zip(&rule.fields).all(|(a, b)| a == b) {
            return rule.type_identity.clone();
        }
    }

    let set: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
    for rule in &rules.key_set_rules {
        match rule.kind.as_str() {
            "any_of" =>
                if rule.fields.iter().any(|f| set.contains(f.as_str())) {
                    return rule.type_identity.clone();
                },
            "all_of_unless" => {
                let all_in = rule.fields.iter().all(|f| set.contains(f.as_str()));
                let blocked = rule.unless_any_of.iter().any(|f| set.contains(f.as_str()));
                if all_in && !blocked {
                    return rule.type_identity.clone();
                }
            }
            other => panic!("unknown key_set rule kind {other:?}"),
        }
    }

    format!("DecodePsbtUnmapped_{}", keys.join("_"))
}

fn rename_decode_psbt_rows(ty: &mut TypeDef, rules: &RpcRules) {
    let source = rules.row_source_name.as_deref().unwrap_or("DecodePsbtRow");
    if ty.name == source && ty.kind == TypeKind::Object {
        let keys = sorted_field_idents(ty);
        let chosen = decode_psbt_type_identity(&keys, rules);
        ty.type_identity = Some(chosen.clone());
        ty.name = chosen;
    }
    if let Some(fields) = ty.fields.as_mut() {
        for f in fields {
            rename_decode_psbt_rows(&mut f.field_type, rules);
        }
    }
    if let Some(uvs) = ty.union_variants.as_mut() {
        for uv in uvs {
            rename_decode_psbt_rows(&mut uv.type_def, rules);
        }
    }
    if let Some(mv) = ty.map_value.as_mut() {
        rename_decode_psbt_rows(mv.as_mut(), rules);
    }
}

fn apply_field_object_renames(ty: &mut TypeDef, rules: &RpcRules) {
    if let Some(fields) = ty.fields.as_mut() {
        for f in fields.iter_mut() {
            let key = f.key.as_ident();
            for rule in &rules.field_object_renames {
                if key == rule.parent_field
                    && f.field_type.kind == TypeKind::Object
                    && f.field_type.name == rule.when_object_name
                {
                    f.field_type.type_identity = Some(rule.type_identity.clone());
                    f.field_type.name = rule.type_identity.clone();
                }
            }
            apply_field_object_renames(&mut f.field_type, rules);
        }
    }
    if let Some(uvs) = ty.union_variants.as_mut() {
        for uv in uvs {
            apply_field_object_renames(&mut uv.type_def, rules);
        }
    }
    if let Some(mv) = ty.map_value.as_mut() {
        apply_field_object_renames(mv.as_mut(), rules);
    }
}

/// Nested `additionalProperties` maps must not share `rust_emit_name` with their value.
///
/// Schema conversion used to name every nested map `{Method}MapValue`, the same label as the
/// leaf object. Codegen then emitted `pub type Foo = BTreeMap<String, Foo>` (E0391).
pub(super) fn uniquify_map_and_value_name_collisions(ty: &mut TypeDef) {
    if let Some(fields) = ty.fields.as_mut() {
        for f in fields {
            uniquify_map_and_value_name_collisions(&mut f.field_type);
        }
    }
    if let Some(uvs) = ty.union_variants.as_mut() {
        for uv in uvs {
            uniquify_map_and_value_name_collisions(&mut uv.type_def);
        }
    }
    if let Some(mv) = ty.map_value.as_mut() {
        uniquify_map_and_value_name_collisions(mv);
    }
    if ty.kind == TypeKind::Map {
        if let Some(mv) = ty.map_value.as_ref() {
            let map_id = ty.rust_emit_name().to_string();
            let val_id = mv.rust_emit_name().to_string();
            if !map_id.is_empty() && map_id != "object" && map_id != "array" && map_id == val_id {
                let renamed = format!("{map_id}Map");
                ty.name = renamed.clone();
                ty.type_identity = Some(renamed);
            }
        }
    }
}

/// Run after converting OpenRPC → IR, before writing or merging canonical IR.
pub(super) fn disambiguate_conflated_type_names(ir: &mut ProtocolIR) {
    let table = rules_table();
    for module in ir.modules_mut() {
        for def in module.definitions_mut() {
            if let ProtocolDef::RpcMethod(rpc) = def {
                let Some(result) = rpc.result.as_mut() else {
                    continue;
                };
                uniquify_map_and_value_name_collisions(result);
                let Some(rules) = table.rpcs.get(rpc.name.as_str()) else {
                    continue;
                };
                match rpc.name.as_str() {
                    "decodepsbt" => rename_decode_psbt_rows(result, rules),
                    "analyzepsbt" | "listdescriptors" => apply_field_object_renames(result, rules),
                    _ => {}
                }
            }
        }
    }
}

fn sorted_field_idents(td: &TypeDef) -> Vec<String> {
    let mut v: Vec<String> = td
        .fields
        .as_ref()
        .map(|fs| fs.iter().map(|f| f.key.as_ident()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use ir::{FieldDef, FieldKey};

    use super::*;

    fn object_named(name: &str, fields: Vec<FieldDef>) -> TypeDef {
        TypeDef {
            name: name.to_string(),
            kind: TypeKind::Object,
            fields: Some(fields),
            protocol_type: Some("object".to_string()),
            ..Default::default()
        }
    }

    fn decode_rules() -> &'static RpcRules {
        rules_table().rpcs.get("decodepsbt").expect("decodepsbt rules")
    }

    #[test]
    fn disambiguation_json_loads() {
        let t = rules_table();
        assert!(t.rpcs.contains_key("decodepsbt"));
        assert!(t.rpcs.contains_key("analyzepsbt"));
        assert!(t.rpcs.contains_key("listdescriptors"));
    }

    #[test]
    fn nested_map_renames_when_value_reuses_map_emit_name() {
        let leaf = object_named(
            "Collide",
            vec![FieldDef {
                key: FieldKey::Named("address".to_string()),
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
            }],
        );
        let mut inner = TypeDef {
            name: "Collide".to_string(),
            kind: TypeKind::Map,
            protocol_type: Some("object-dynamic".to_string()),
            type_identity: Some("Collide".to_string()),
            map_value: Some(Box::new(leaf)),
            map_key_protocol_type: Some("string".to_string()),
            ..Default::default()
        };
        uniquify_map_and_value_name_collisions(&mut inner);
        assert_eq!(inner.rust_emit_name(), "CollideMap");
        assert_eq!(inner.map_value_type().unwrap().rust_emit_name(), "Collide");
    }

    #[test]
    fn decode_psbt_global_xpub_shape() {
        let keys = vec!["master_fingerprint".to_string(), "path".to_string(), "xpub".to_string()];
        assert_eq!(decode_psbt_type_identity(&keys, decode_rules()), "DecodePsbtGlobalXpub");
    }

    #[test]
    fn decode_psbt_musig2_partial_sigs_matches_lexicographic_key_order() {
        let keys = vec![
            "aggregate_pubkey".to_string(),
            "leaf_hash".to_string(),
            "partial_sig".to_string(),
            "participant_pubkey".to_string(),
        ];
        assert_eq!(decode_psbt_type_identity(&keys, decode_rules()), "DecodePsbtMusig2PartialSigs");
    }

    #[test]
    fn decode_psbt_renames_row_in_tree() {
        let mut ty = object_named(
            "object",
            vec![FieldDef {
                key: FieldKey::Named("global_xpubs".to_string()),
                field_type: TypeDef {
                    name: "array".to_string(),
                    kind: TypeKind::Array,
                    fields: Some(vec![FieldDef {
                        key: FieldKey::Named("field_0".to_string()),
                        field_type: object_named(
                            "DecodePsbtRow",
                            vec![
                                FieldDef {
                                    key: FieldKey::Named("xpub".to_string()),
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
                                FieldDef {
                                    key: FieldKey::Named("master_fingerprint".to_string()),
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
                                },
                                FieldDef {
                                    key: FieldKey::Named("path".to_string()),
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
                            ],
                        ),
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
                },
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }],
        );
        rename_decode_psbt_rows(&mut ty, decode_rules());
        let inner =
            &ty.fields.as_ref().expect("fields")[0].field_type.fields.as_ref().expect("arr fields")
                [0]
            .field_type;
        assert_eq!(inner.name, "DecodePsbtGlobalXpub");
        assert_eq!(inner.type_identity.as_deref(), Some("DecodePsbtGlobalXpub"));
    }

    #[test]
    fn analyze_psbt_missing_split() {
        let rules = rules_table().rpcs.get("analyzepsbt").expect("analyzepsbt");
        let mut row = object_named(
            "AnalyzePsbtRow",
            vec![
                FieldDef {
                    key: FieldKey::Named("has_utxo".to_string()),
                    field_type: TypeDef {
                        name: "boolean".to_string(),
                        kind: TypeKind::Primitive,
                        protocol_type: Some("boolean".to_string()),
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
                FieldDef {
                    key: FieldKey::Named("missing".to_string()),
                    field_type: object_named("AnalyzePsbtRow", vec![]),
                    required: false,
                    description: String::new(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                },
            ],
        );
        apply_field_object_renames(&mut row, rules);
        let missing = row.fields.as_ref().expect("fields")[1].field_type.name.clone();
        assert_eq!(missing, "AnalyzePsbtMissing");
        assert_eq!(
            row.fields.as_ref().expect("fields")[1].field_type.type_identity.as_deref(),
            Some("AnalyzePsbtMissing")
        );
        assert_eq!(row.fields.as_ref().expect("fields")[0].field_type.name, "boolean");
    }
}
