//! IR Validation
//!
//! Validates the ProtocolIR for correctness and consistency.
//! Performs various checks to ensure the IR is well-formed before
//! proceeding to code generation.

use ir::{AccessLevel, ProtocolDef, ProtocolIR, RpcDef, TypeDef, TypeKind};

use crate::{CompilerContext, CompilerPhase, PhaseResult};

/// IR Validator
pub struct IrValidator;

impl Default for IrValidator {
    fn default() -> Self { Self::new() }
}

impl IrValidator {
    /// Create a new IR validator
    pub fn new() -> Self { Self }
}

impl IrValidator {
    /// Validate a ProtocolIR and return validation errors
    pub fn validate(&self, ir: &ProtocolIR) -> Vec<String> {
        let mut errors = Vec::new();

        // 1) Unique RPC names across modules
        {
            use std::collections::HashSet;
            let mut seen = HashSet::new();
            for m in ir.modules() {
                for d in m.definitions() {
                    if let ProtocolDef::RpcMethod(r) = d {
                        if !seen.insert(r.name.clone()) {
                            errors.push(format!("Duplicate RPC name: {}", r.name));
                        }
                    }
                }
            }
        }

        // 2) Per-RPC checks
        for m in ir.modules() {
            for d in m.definitions() {
                if let ProtocolDef::RpcMethod(rpc) = d {
                    self.validate_rpc(rpc, &mut errors);
                }
            }
        }

        errors
    }

    fn validate_rpc(&self, rpc: &RpcDef, errors: &mut Vec<String>) {
        // params: non-empty names, unique within method
        {
            use std::collections::HashSet;
            let mut seen = HashSet::new();
            for p in &rpc.params {
                if p.name.trim().is_empty() {
                    errors.push(format!("RPC `{}` has a param with empty name", rpc.name));
                }
                if !seen.insert(p.name.clone()) {
                    errors.push(format!("RPC `{}` duplicate param `{}`", rpc.name, p.name));
                }
                self.validate_type(
                    rpc,
                    &p.name,
                    &p.param_type,
                    /*is_result=*/ false,
                    p.required,
                    errors,
                );
            }
        }

        // result type (if any)
        if let Some(t) = &rpc.result {
            self.validate_type(rpc, "__result__", t, /*is_result=*/ true, true, errors);
        }

        let expected = expected_access_level(&rpc.category, &rpc.name);
        if rpc.access_level != expected {
            errors.push(format!(
                "RPC `{}` access level {:?} does not match expected {:?}",
                rpc.name, rpc.access_level, expected
            ));
        }
    }

    fn validate_type(
        &self,
        rpc: &RpcDef,
        field: &str,
        t: &TypeDef,
        _is_result: bool,
        required: bool,
        errors: &mut Vec<String>,
    ) {
        // Custom must have base_type
        if matches!(t.kind, TypeKind::Custom) && t.base_type.is_none() {
            errors.push(format!(
                "RPC `{}` field `{}`: Custom type must set base_type",
                rpc.name, field
            ));
        }

        // Optional kinds are not allowed in IR hot path (optional should be expressed by param.required=false)
        if matches!(t.kind, TypeKind::Optional) && required {
            errors.push(format!(
                "RPC `{}` field `{}`: Optional kind with required=true is invalid",
                rpc.name, field
            ));
        }

        // Primitive types must have non-empty name
        if matches!(t.kind, TypeKind::Primitive) && t.name.trim().is_empty() {
            errors.push(format!(
                "RPC `{}` field `{}`: Primitive type name is empty",
                rpc.name, field
            ));
        }
    }
}

// Access level calculation. Keep in sync with pipeline/codegen logic.
fn expected_access_level(category: &str, method_name: &str) -> AccessLevel {
    if category.to_lowercase() == "hidden" {
        let name_lower = method_name.to_lowercase();

        if name_lower.starts_with("generate")
            || name_lower.starts_with("mock")
            || name_lower == "setmocktime"
        {
            return AccessLevel::Testing;
        }

        if name_lower.starts_with("getorphan")
            || name_lower.starts_with("getrawaddrman")
            || name_lower.starts_with("echo")
            || name_lower == "sendmsgtopeer"
        {
            return AccessLevel::Internal;
        }

        if name_lower == "invalidateblock"
            || name_lower == "reconsiderblock"
            || name_lower.starts_with("addconnection")
            || name_lower.starts_with("addpeeraddress")
        {
            return AccessLevel::Advanced;
        }
    }

    AccessLevel::Public
}

impl CompilerPhase for IrValidator {
    fn name(&self) -> &str { "IrValidator" }

    fn description(&self) -> &str { "Validate ProtocolIR for correctness and consistency" }

    fn run(&self, ctx: &mut CompilerContext) -> PhaseResult {
        // Validate the IR
        let errors = self.validate(&ctx.ir);

        if errors.is_empty() {
            ctx.diagnostics
                .stats
                .entry("ir_validation_errors".to_string())
                .and_modify(|e| *e += 0)
                .or_insert(0);
        } else {
            // Add errors to context
            for error in &errors {
                ctx.add_error(error.clone());
            }

            ctx.diagnostics
                .stats
                .entry("ir_validation_errors".to_string())
                .and_modify(|e| *e += errors.len())
                .or_insert(errors.len());
        }

        Ok(())
    }
}
