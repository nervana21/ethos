use adapters::{clear_fallback_events, fallback_events_snapshot};
use ir::test_utils::{field, field_anon, primitive_type};

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
    let elem_ty = primitive_type("string", Some("string".to_string()));

    // Element encoded as anonymous positional field_0.
    let result_ty = TypeDef {
        name: "array".to_string(),
        description: String::new(),
        kind: TypeKind::Array,
        fields: Some(vec![field_anon(0, elem_ty, true)]),
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
    assert!(code.contains("pub value: Vec<String>"), "expected Vec<String> field, got:\n{code}");
}

#[test]
fn array_wrapper_recognizes_named_field_0_element() {
    let version = ProtocolVersion::from_str("30.0.0").unwrap();
    let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

    // IR: top-level array of primitive strings with a synthetic Named(\"field_0\") key.
    let elem_ty = primitive_type("string", Some("string".to_string()));

    let result_ty = TypeDef {
        name: "array".to_string(),
        description: String::new(),
        kind: TypeKind::Array,
        fields: Some(vec![field("field_0", elem_ty, true)]),
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
        RpcDef { name: "decodepsbt".to_string(), result: Some(result_ty), ..Default::default() };

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

    let method =
        RpcDef { name: "decodepsbt".to_string(), result: Some(result_ty), ..Default::default() };

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

    let method =
        RpcDef { name: "getrawmempool".to_string(), result: Some(result_ty), ..Default::default() };

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
        "IR disambiguation key hash_1 is emitted for distinct wire shapes, got:\n{code}"
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
fn lookup_or_null_emits_option_shaped_object_response() {
    let version = ProtocolVersion::from_str("30.0.0").unwrap();
    let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

    let object = TypeDef {
        name: "GetTxOutObject".to_string(),
        kind: TypeKind::Object,
        fields: Some(vec![ir::FieldDef {
            key: ir::FieldKey::Named("bestblock".to_string()),
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
            force_optional: None,
        }]),
        protocol_type: Some("object".to_string()),
        ..Default::default()
    };
    let union_td = TypeDef {
        name: "GetTxOutResult".to_string(),
        kind: TypeKind::Union,
        union_variants: Some(vec![
            ir::UnionVariantDef {
                name: "Null".to_string(),
                description: "not found".to_string(),
                condition: None,
                type_def: TypeDef {
                    name: "none".to_string(),
                    kind: TypeKind::Primitive,
                    protocol_type: Some("none".to_string()),
                    ..Default::default()
                },
            },
            ir::UnionVariantDef {
                name: "Object".to_string(),
                description: "found".to_string(),
                condition: None,
                type_def: object,
            },
        ]),
        protocol_type: Some("union".to_string()),
        ..Default::default()
    };
    let method =
        RpcDef { name: "gettxout".to_string(), result: Some(union_td), ..Default::default() };

    assert!(VersionSpecificResponseTypeGenerator::is_lookup_or_null_result(
        method.result.as_ref().unwrap()
    ));
    let code = gen
        .generate_method_response(&method)
        .expect("generation must succeed")
        .expect("response must be generated");
    assert!(
        code.contains("pub struct GetTxOutResponse"),
        "expected object struct as GetTxOutResponse, got:\n{code}"
    );
    assert!(
        !code.contains("pub enum GetTxOutResponse"),
        "lookup-or-null must not emit untagged enum, got:\n{code}"
    );
    assert!(code.contains("Wire method: `gettxout`"), "expected method→type rustdoc, got:\n{code}");
}

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
            force_optional: None,
        }]),
        protocol_type: Some("object".to_string()),
        ..Default::default()
    };

    let method =
        RpcDef { name: "wrapper_demo".to_string(), result: Some(wrapper), ..Default::default() };

    let files = gen.generate(&[method]).expect("generate");
    let code =
        files.iter().find(|(name, _)| name == "responses.rs").expect("responses.rs").1.clone();

    assert!(
        code.contains("pub child: Option<Box<RecursiveRow>>"),
        "nested recursive IR object needs Box, got:\n{code}"
    );
}

#[test]
fn union_branch_merge_deep_merges_nested_prevout() {
    let version = ProtocolVersion::from_str("30.0.0").unwrap();
    let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());

    let string_ty = TypeDef {
        name: "string".to_string(),
        kind: TypeKind::Primitive,
        protocol_type: Some("string".to_string()),
        ..Default::default()
    };
    let vin_v2 = TypeDef {
        name: "GetBlockVin".to_string(),
        type_identity: Some("GetBlockVin".to_string()),
        kind: TypeKind::Object,
        fields: Some(vec![ir::FieldDef {
            key: ir::FieldKey::Named("txid".to_string()),
            field_type: string_ty.clone(),
            required: false,
            description: String::new(),
            default_value: None,
            version_added: None,
            version_removed: None,
            force_optional: None,
        }]),
        protocol_type: Some("object".to_string()),
        ..Default::default()
    };
    let prevout = TypeDef {
        name: "GetBlockPrevout".to_string(),
        type_identity: Some("GetBlockPrevout".to_string()),
        kind: TypeKind::Object,
        fields: Some(vec![ir::FieldDef {
            key: ir::FieldKey::Named("height".to_string()),
            field_type: TypeDef {
                name: "number".to_string(),
                kind: TypeKind::Primitive,
                protocol_type: Some("number".to_string()),
                ..Default::default()
            },
            required: true,
            description: String::new(),
            default_value: None,
            version_added: None,
            version_removed: None,
            force_optional: None,
        }]),
        protocol_type: Some("object".to_string()),
        ..Default::default()
    };
    let vin_v3 = TypeDef {
        name: "GetBlockVin".to_string(),
        type_identity: Some("GetBlockVin".to_string()),
        kind: TypeKind::Object,
        fields: Some(vec![
            ir::FieldDef {
                key: ir::FieldKey::Named("txid".to_string()),
                field_type: string_ty.clone(),
                required: false,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                force_optional: None,
            },
            ir::FieldDef {
                key: ir::FieldKey::Named("prevout".to_string()),
                field_type: prevout,
                required: false,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                force_optional: None,
            },
        ]),
        protocol_type: Some("object".to_string()),
        ..Default::default()
    };

    let merged = gen.merge_object_type_defs_for_union_branches(&[vin_v2, vin_v3], "getblock");
    let keys: Vec<_> = merged.fields.as_ref().unwrap().iter().map(|f| f.key.as_ident()).collect();
    assert!(
        keys.iter().any(|k| k == "prevout"),
        "deep-merge must keep verbosity-3 prevout, got {keys:?}"
    );
    let prevout_field =
        merged.fields.as_ref().unwrap().iter().find(|f| f.key.as_ident() == "prevout").unwrap();
    assert!(!prevout_field.required, "prevout only on one arm ⇒ optional after merge");
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

    let method =
        RpcDef { name: "getrawaddrman".to_string(), result: Some(outer_map), ..Default::default() };

    let files = gen.generate(&[method]).expect("generate");
    let code =
        files.iter().find(|(name, _)| name == "responses.rs").expect("responses.rs").1.clone();

    assert!(
        code.contains("pub struct GetRawAddrManMapValue"),
        "leaf object must be a struct, got:\n{code}"
    );
    assert!(code.contains("pub address:"), "leaf object must keep address field, got:\n{code}");
    assert!(
        !code.contains("pub type GetRawAddrManMapValue = BTreeMap<String, GetRawAddrManMapValue>"),
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

    assert!(code.contains("#[serde(transparent)]"), "expected transparent wrapper, got:\n{code}");
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
    let workspace_root = path::workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"), 2);
    let ir_path = path::canonical_bitcoin_ir_path(&workspace_root);
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
    let workspace_root = path::workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"), 2);
    let ir_path = path::canonical_bitcoin_ir_path(&workspace_root);
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

#[test]
fn amount_fields_emit_btc_float_serde_pair() {
    let version = ProtocolVersion::from_str("32.0.0").expect("protocol version");
    let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());
    let method = RpcDef {
        name: "gettxout".to_string(),
        result: Some(TypeDef {
            name: "GetTxOutResult".to_string(),
            kind: TypeKind::Object,
            fields: Some(vec![
                field(
                    "value",
                    TypeDef {
                        name: "amount".to_string(),
                        kind: TypeKind::Primitive,
                        protocol_type: Some("amount".to_string()),
                        ..Default::default()
                    },
                    true,
                ),
                field(
                    "coinbase",
                    TypeDef {
                        name: "boolean".to_string(),
                        kind: TypeKind::Primitive,
                        protocol_type: Some("boolean".to_string()),
                        ..Default::default()
                    },
                    false,
                ),
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let code = gen
        .generate_method_response(&method)
        .expect("generate")
        .expect("response");
    assert!(
        code.contains("serialize_with = \"amount_to_btc_float\""),
        "expected BTC float serializer on required Amount:\n{code}"
    );
    assert!(
        code.contains("deserialize_with = \"amount_from_btc_float\""),
        "expected Amount deserializer:\n{code}"
    );
    assert!(
        code.contains("skip_serializing_if = \"Option::is_none\""),
        "expected Option omit:\n{code}"
    );
}

#[test]
fn rpc_prelude_exports_floresta_consumer_aliases() {
    let workspace_root = path::workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"), 2);
    let ir_path = path::canonical_bitcoin_ir_path(&workspace_root);
    let ir = ir::ProtocolIR::from_file(&ir_path)
        .unwrap_or_else(|e| panic!("load IR {}: {e}", ir_path.display()));
    let methods: Vec<RpcDef> = ir.get_rpc_methods().into_iter().cloned().collect();
    let version = ProtocolVersion::from_str("32.0.0").expect("protocol version");
    let gen = VersionSpecificResponseTypeGenerator::new(version, "bitcoin_core".to_string());
    let files = gen.generate(&methods).expect("generate responses");
    let responses = files
        .iter()
        .find(|(name, _)| name == "responses.rs")
        .map(|(_, content)| content)
        .expect("responses.rs");

    assert!(
        responses.contains("pub use rpc_prelude as aliases;"),
        "expected aliases module alias:\n{responses}"
    );

    // Exact Floresta shim surface (consumer short-name table), minus identity re-exports.
    let expected = [
        ("GetBlockVerboseOne", "GetBlockResponseGetBlockVerbosity1"),
        ("GetBlockHeaderVerbose", "GetBlockHeaderResponseGetBlockHeaderVerboseTrue"),
        ("GetRawTransactionVerbose", "GetRawTransactionResponseGetRawTransactionVerbosity1"),
        ("GetTxOut", "GetTxOutResponse"),
        ("ScriptPubKey", "GetTxOutScriptPubKey"),
        ("GetBlockchainInfo", "GetBlockchainInfoResponse"),
        ("GetNetworkInfo", "GetNetworkInfoResponse"),
        ("GetNetworkInfoNetwork", "GetNetworkInfoNetworks"),
        ("GetAddrManInfo", "GetAddrManInfoResponse"),
        ("AddrManInfoNetwork", "GetAddrManInfoMapValue"),
        ("GetDeploymentInfo", "GetDeploymentInfoResponse"),
        ("DeploymentInfo", "GetDeploymentInfoMapValue"),
        ("RawTransactionScriptPubKey", "GetRawTransactionVerbosity1ScriptPubKey"),
        ("ScriptSig", "GetRawTransactionVerbosity1ScriptSig"),
        ("RawTransactionInput", "GetRawTransactionVerbosity1Vin"),
        ("RawTransactionOutput", "GetRawTransactionVerbosity1Vout"),
    ];
    for (short, long) in expected {
        let line = format!("pub use super::{long} as {short};");
        assert!(responses.contains(&line), "missing consumer alias `{line}` in rpc_prelude");
    }
}
