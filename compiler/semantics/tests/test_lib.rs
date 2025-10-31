use ethos_semantics::{
    SemanticAnalyzer, SemanticEntity, SemanticGraph, SemanticInvariant, SemanticKind,
    SemanticRelation,
};
use ir::{
    AccessLevel, ParamDef, ProtocolDef, ProtocolIR, ProtocolModule, RpcDef, TypeDef, TypeKind,
};

/// Helper function to create a test TypeDef
fn create_test_type(name: &str, kind: TypeKind) -> TypeDef {
    TypeDef {
        name: name.into(),
        description: String::new(),
        kind,
        fields: None,
        variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
    }
}

/// Helper function to create a test RpcDef
fn create_test_rpc(name: &str, params: Vec<ParamDef>, result: Option<TypeDef>) -> RpcDef {
    RpcDef {
        name: name.into(),
        description: String::new(),
        params,
        result,
        category: "test".into(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    }
}

/// Helper function to create a test ParamDef
fn create_test_param(name: &str, param_type: TypeDef) -> ParamDef {
    ParamDef {
        name: name.into(),
        param_type,
        required: true,
        description: String::new(),
        default_value: None,
    }
}

#[test]
fn test_from_ir() {
    let empty_ir = ProtocolIR::new(vec![]);
    let empty_graph = SemanticAnalyzer::from_ir(&empty_ir)
        .expect("Failed to create semantic analyzer from empty IR");
    assert!(empty_graph.entities.is_empty());
    assert!(empty_graph.relations.is_empty());

    let foo_type = create_test_type("Foo", TypeKind::Object);
    let bar_type = create_test_type("Bar", TypeKind::Object);
    let string_type = create_test_type("String", TypeKind::Primitive);
    let rpc_method = create_test_rpc(
        "test_method",
        vec![
            create_test_param("param1", foo_type.clone()),
            create_test_param("param2", string_type.clone()),
        ],
        Some(bar_type.clone()),
    );
    let module = ProtocolModule::new(
        "test_module".into(),
        "Test module".into(),
        vec![
            ProtocolDef::Type(foo_type.clone()),
            ProtocolDef::Type(bar_type.clone()),
            ProtocolDef::RpcMethod(rpc_method.clone()),
        ],
    );
    let ir = ProtocolIR::new(vec![module.clone()]);
    let graph = SemanticAnalyzer::from_ir(&ir).expect("Failed to create semantic analyzer from IR");
    assert!(graph
        .entities
        .iter()
        .any(|e| e.name == "test_method" && e.kind == SemanticKind::RpcMethod));
    assert!(graph.entities.iter().any(|e| e.name == "Foo" && e.kind == SemanticKind::TypeDef));
    assert!(graph.entities.iter().any(|e| e.name == "Bar" && e.kind == SemanticKind::TypeDef));
    // The primitive type String should not be added as an entity
    assert!(!graph.entities.iter().any(|e| e.name == "String"));
    assert!(graph.relations.iter().any(|r| r.from == "test_method" && r.to == "Foo"));
    assert!(graph.relations.iter().any(|r| r.from == "test_method" && r.to == "Bar"));
    // No relation should exist for the primitive String type
    assert!(!graph.relations.iter().any(|r| r.from == "test_method" && r.to == "String"));

    let duplicate_module = ProtocolModule::new(
        "duplicate_module".into(),
        "Duplicate module".into(),
        vec![ProtocolDef::Type(foo_type.clone()), ProtocolDef::RpcMethod(rpc_method.clone())],
    );
    let ir_with_duplicates = ProtocolIR::new(vec![module.clone(), duplicate_module]);
    let graph_with_duplicates = SemanticAnalyzer::from_ir(&ir_with_duplicates)
        .expect("Failed to create semantic analyzer from IR with duplicates");

    let foo_count = graph_with_duplicates.entities.iter().filter(|e| e.name == "Foo").count();
    let method_count =
        graph_with_duplicates.entities.iter().filter(|e| e.name == "test_method").count();
    assert_eq!(foo_count, 1);
    assert_eq!(method_count, 1);
    let foo_relation_count = graph_with_duplicates
        .relations
        .iter()
        .filter(|r| r.from == "test_method" && r.to == "Foo")
        .count();
    assert_eq!(foo_relation_count, 1);
}

#[test]
fn test_default_invariants() {
    let invariants = SemanticAnalyzer::default_invariants();
    assert!(!invariants.is_empty());
    assert!(invariants.iter().any(|inv| inv.name == "All relations point to known entities"));

    let mut valid_graph = SemanticGraph::default();
    valid_graph
        .entities
        .push(SemanticEntity { name: "method1".into(), kind: SemanticKind::RpcMethod });
    valid_graph.entities.push(SemanticEntity { name: "Type1".into(), kind: SemanticKind::TypeDef });
    valid_graph.relations.push(SemanticRelation { from: "method1".into(), to: "Type1".into() });
    for invariant in &invariants {
        assert!(
            (invariant.check)(&valid_graph),
            "Valid graph should pass invariant: {}",
            invariant.name
        );
    }

    let mut invalid_graph = SemanticGraph::default();
    invalid_graph
        .entities
        .push(SemanticEntity { name: "method1".into(), kind: SemanticKind::RpcMethod });
    invalid_graph
        .relations
        .push(SemanticRelation { from: "method1".into(), to: "MissingType".into() });
    for invariant in &invariants {
        if invariant.name == "All relations point to known entities" {
            assert!(
                !(invariant.check)(&invalid_graph),
                "Invalid graph should fail invariant: {}",
                invariant.name
            );
        }
    }
}

#[test]
fn test_diagnostics_for_invariants() {
    let invariants = SemanticAnalyzer::default_invariants();

    let mut valid_graph = SemanticGraph::default();
    valid_graph
        .entities
        .push(SemanticEntity { name: "method1".into(), kind: SemanticKind::RpcMethod });
    valid_graph.entities.push(SemanticEntity { name: "Type1".into(), kind: SemanticKind::TypeDef });
    valid_graph.relations.push(SemanticRelation { from: "method1".into(), to: "Type1".into() });
    let diagnostics = valid_graph.diagnostics_for_invariants(&invariants);
    assert!(diagnostics.is_empty(), "Valid graph should produce no diagnostics");

    let mut invalid_graph = SemanticGraph::default();
    invalid_graph
        .entities
        .push(SemanticEntity { name: "method1".into(), kind: SemanticKind::RpcMethod });
    invalid_graph
        .relations
        .push(SemanticRelation { from: "method1".into(), to: "MissingType".into() });
    let diagnostics = invalid_graph.diagnostics_for_invariants(&invariants);
    assert!(!diagnostics.is_empty(), "Invalid graph should produce diagnostics");
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.invariant, "All relations point to known entities");
    assert_eq!(diagnostic.message, "Every relation target must exist as an entity");
    assert!(diagnostic.related_entities.contains(&"method1 -> MissingType".to_string()));

    let custom_invariant = SemanticInvariant {
        name: "Custom invariant".into(),
        description: "Custom test invariant".into(),
        check: |_| false, // Always fails
    };
    let custom_invariants = vec![invariants[0].clone(), custom_invariant];
    let diagnostics = invalid_graph.diagnostics_for_invariants(&custom_invariants);
    assert_eq!(diagnostics.len(), 2, "Should produce diagnostics for both failed invariants");
    let invariant_names: Vec<&str> = diagnostics.iter().map(|d| d.invariant.as_str()).collect();
    assert!(invariant_names.contains(&"All relations point to known entities"));
    assert!(invariant_names.contains(&"Custom invariant"));
}
