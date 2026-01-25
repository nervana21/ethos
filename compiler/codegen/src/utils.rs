// codegen/src/utils.rs

use serde_json::{self, Value};
use types::Argument;

// Embed normalization presets at compile time; missing files will hard-fail build
const BITCOIN_NORMALIZATION_JSON: &str =
    include_str!("../resources/adapters/normalization/bitcoin.json");
const LIGHTNING_NORMALIZATION_JSON: &str =
    include_str!("../resources/adapters/normalization/lightning.json");

/// Strict registry-driven conversion: adapter-specific RPC → canonical → snake_case
///
/// - protocol: "bitcoin_core" | "core_lightning"
/// - rpc_method: adapter-specific RPC method (e.g., "getblockchaininfo", "getinfo")
///
/// Errors if the preset is missing or the rpc_method has no mapping.
pub fn protocol_rpc_method_to_rust_name(
    protocol: &str,
    rpc_method: &str,
) -> Result<String, String> {
    let canonical = canonical_from_adapter_method(protocol, rpc_method)?;
    Ok(pascal_to_snake_case(&canonical))
}

/// Convert camelCase to snake_case
pub fn rpc_method_to_rust_name(rpc_method: &str) -> String {
    if rpc_method.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
        return pascal_to_snake_case(rpc_method);
    }

    let sanitized = sanitize_external_identifier(rpc_method);
    camel_to_snake_case(&sanitized)
}

/// Resolve canonical PascalCase name from adapter-specific RPC using normalization presets
pub fn canonical_from_adapter_method(protocol: &str, rpc_method: &str) -> Result<String, String> {
    let (preset_json_str, impl_key) = match protocol {
        "bitcoin_core" => (BITCOIN_NORMALIZATION_JSON, "bitcoin_core"),
        "core_lightning" => (LIGHTNING_NORMALIZATION_JSON, "core_lightning"),
        other =>
            return Err(format!(
                "Unsupported protocol '{}'. Supported: bitcoin_core, core_lightning",
                other
            )),
    };

    let preset: Value = serde_json::from_str(preset_json_str)
        .map_err(|e| format!("Failed to parse normalization preset for {}: {}", protocol, e))?;

    let mappings = preset
        .get("method_mappings")
        .and_then(|mm| mm.get(impl_key))
        .and_then(|v| v.as_object())
        .ok_or_else(|| format!(
            "Normalization preset for '{}' missing method_mappings. File must define method_mappings.{}",
            protocol,
            impl_key
        ))?;

    // Build reverse map: adapter-specific -> canonical (PascalCase)
    for (canonical, adapter_specific_val) in mappings.iter() {
        if let Some(adapter_specific) = adapter_specific_val.as_str() {
            if adapter_specific == rpc_method {
                return Ok(canonical.to_string());
            }
        }
    }

    Err(format!(
        "Unmapped RPC method '{}' for '{}'. Add it to resources/adapters/normalization/{}.json under method_mappings.{}.",
        rpc_method,
        protocol,
        if protocol == "bitcoin_core" { "bitcoin" } else { "lightning" },
        impl_key
    ))
}

/// Convert camelCase to snake_case
fn camel_to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = input.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            // Check if previous character is lowercase or if this is the start of a new word
            if let Some(prev) = chars.get(i - 1) {
                if prev.is_lowercase()
                    || (i > 1 && chars.get(i - 2).is_some_and(|p| p.is_lowercase()))
                {
                    result.push('_');
                }
            }
        }
        result.push(c.to_lowercase().next().unwrap_or(*c));
    }

    result
}

/// Converts a PascalCase string to snake_case
///
/// This function is used to convert RPC method names from their PascalCase type name form
/// (e.g., `GetBlockchainInfo`, `AddNode`) to idiomatic Rust snake_case function names
/// (e.g., `get_blockchain_info`, `add_node`).
///
/// # Examples
/// ```
/// use ethos_codegen::utils::pascal_to_snake_case;
/// assert_eq!(pascal_to_snake_case("GetBlockchainInfo"), "get_blockchain_info");
/// assert_eq!(pascal_to_snake_case("AddNode"), "add_node");
/// assert_eq!(pascal_to_snake_case("GetBalance"), "get_balance");
/// ```
pub fn pascal_to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let chars = input.chars();

    for c in chars {
        if c.is_uppercase() && !result.is_empty() {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }

    result
}

/// Capitalize the first letter of a string
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Convert snake_case to PascalCase
pub fn snake_to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// Sanitize field names to be valid Rust identifiers
pub fn sanitize_field_name(name: &str) -> String {
    // Handle reserved keywords
    match name {
        "type" => "r#type".to_string(),
        "self" => "self_".to_string(),
        "super" => "super_".to_string(),
        "crate" => "crate_".to_string(),
        _ => {
            // Replace hyphens with underscores and remove other invalid characters
            let sanitized = name.replace('-', "_");
            // Remove any remaining invalid characters (keep only alphanumeric and underscores)
            sanitized.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect()
        }
    }
}

/// Sanitizes external identifiers (e.g. RPC schemas) to be valid Rust identifiers
pub fn sanitize_external_identifier(name: &str) -> String {
    // Handle reserved keywords
    match name {
        "type" => "r#type".to_string(),
        "self" => "self_".to_string(),
        "super" => "super_".to_string(),
        "crate" => "crate_".to_string(),
        _ => {
            // Replace hyphens with underscores and remove other invalid characters
            let sanitized = name.replace('-', "_");
            // Remove any remaining invalid characters (keep only alphanumeric and underscores)
            sanitized.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect()
        }
    }
}

/// Check if a method needs parameter reordering
pub fn needs_parameter_reordering(args: &[Argument]) -> bool {
    // Simple heuristic: if we have more than 3 parameters, use a struct
    args.len() > 3
}

/// Reorder arguments for better Rust API
pub fn reorder_arguments_for_rust_signature(args: &[Argument]) -> (Vec<Argument>, Vec<usize>) {
    // For now, just return the original order
    // In the future, this could implement more sophisticated reordering
    let mapping: Vec<usize> = (0..args.len()).collect();
    (args.to_vec(), mapping)
}

/// Generate mod.rs content
pub fn generate_mod_rs(
    _clients_dir: &std::path::PathBuf,
    _clients_dir_name: &str,
) -> std::io::Result<()> {
    // For now, just return Ok
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pascal_to_snake_case() {
        assert_eq!(pascal_to_snake_case("GetBlockchainInfo"), "get_blockchain_info");
        assert_eq!(pascal_to_snake_case("AddNode"), "add_node");
        assert_eq!(pascal_to_snake_case("GetBalance"), "get_balance");
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize("world"), "World");
    }
}
