// SPDX-License-Identifier: CC0-1.0

//! Shared helpers for detecting semantic `FeeRate` usage in RPC definitions.

use ir::RpcDef;
use types::type_adapter::TypeAdapter;
use types::TypeRegistry;

/// Returns true if any parameter across the provided methods maps to the shared `FeeRate` type.
///
/// This uses the protocol-specific `TypeAdapter` to resolve the semantic base type for each
/// parameter, ensuring we only rely on the adapter's mapping rather than ad-hoc name checks.
pub fn methods_use_fee_rate<'a, I>(methods: I, adapter: &dyn TypeAdapter) -> bool
where
    I: IntoIterator<Item = &'a RpcDef>,
{
    methods.into_iter().any(|m| {
        m.params.iter().any(|p| {
            let protocol_type = p.param_type.protocol_type.as_ref().unwrap_or_else(|| {
                panic!(
                    "Parameter '{}' in method '{}' is missing protocol_type. Rust type name is '{}'. \
                    All parameters must have protocol_type set for proper type categorization.",
                    p.name, m.name, p.param_type.name
                )
            });
            let arg = types::Argument {
                names: vec![p.name.clone()],
                type_: protocol_type.clone(),
                required: p.required,
                description: p.description.clone(),
                oneline_description: String::new(),
                also_positional: false,
                hidden: false,
                type_str: None,
            };
            let (base_ty, _) =
                TypeRegistry::map_argument_type_with_adapter(&arg, adapter);
            base_ty == "FeeRate"
        })
    })
}

/// Returns true if any parameter across the provided methods is a `FeeRate`-typed parameter
/// named `"maxfeerate"` (used for custom BTC/kvB serde).
///
/// Uses the same protocol-specific `TypeAdapter` as `methods_use_fee_rate`.
pub fn methods_use_maxfeerate<'a, I>(methods: I, adapter: &dyn TypeAdapter) -> bool
where
    I: IntoIterator<Item = &'a RpcDef>,
{
    methods.into_iter().any(|m| {
        m.params.iter().any(|p| {
            let protocol_type = p.param_type.protocol_type.as_ref().unwrap_or_else(|| {
                panic!(
                    "Parameter '{}' in method '{}' is missing protocol_type. Rust type name is '{}'. \
                    All parameters must have protocol_type set for proper type categorization.",
                    p.name, m.name, p.param_type.name
                )
            });
            let arg = types::Argument {
                names: vec![p.name.clone()],
                type_: protocol_type.clone(),
                required: p.required,
                description: p.description.clone(),
                oneline_description: String::new(),
                also_positional: false,
                hidden: false,
                type_str: None,
            };
            let (base_ty, _) =
                TypeRegistry::map_argument_type_with_adapter(&arg, adapter);
            base_ty == "FeeRate" && p.name == "maxfeerate"
        })
    })
}
