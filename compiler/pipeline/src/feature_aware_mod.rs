//! Feature-aware mod.rs generation
//!
//! Generates mod.rs files with conditional compilation based on features

use std::path::Path;

use codegen::generators::client_trait::MethodTemplate;
use ir::RpcDef;
use semantics::method_categorization::{group_methods_by_category, MethodCategory};
use types::Implementation;

use crate::PipelineError;

/// Remove trailing whitespace from each line and trim empty lines at EOF.
/// Always ensures the returned string ends with a single newline when not empty.
fn trim_trailing_whitespace(content: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.trim_end().to_string()).collect();

    while matches!(lines.last(), Some(line) if line.is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

/// Generate feature-aware mod.rs for methods directory
pub fn generate_methods_mod_rs(output_dir: &Path, methods: &[RpcDef]) -> Result<(), PipelineError> {
    let groups = group_methods_by_category(methods);

    let mut content = String::new();
    content.push_str("//! RPC method implementations organized by category\n");
    content.push_str("//!\n");
    content.push_str("//! This module contains all RPC method implementations,\n");
    content.push_str("//! organized by semantic category with feature-gated compilation.\n\n");

    // Generate conditional module declarations
    let mut categories: Vec<MethodCategory> = groups.keys().cloned().collect();
    categories.sort_by_key(|c| c.display_name());
    for category in categories {
        let feature_name = category.feature_name();
        let dir_name = category.dir_name();
        content.push_str(&format!("#[cfg(feature = \"{}\")]\n", feature_name));
        content.push_str(&format!("pub mod {};\n", dir_name));
        content.push_str(&format!("#[cfg(feature = \"{}\")]\n", feature_name));
        content.push_str(&format!("pub use {}::*;\n\n", dir_name));
    }

    // Write to methods/mod.rs
    let methods_dir = output_dir.join("methods");
    std::fs::create_dir_all(&methods_dir)?;
    let mod_rs_path = methods_dir.join("mod.rs");
    let content = trim_trailing_whitespace(&content);
    std::fs::write(mod_rs_path, content)?;

    Ok(())
}

/// Generate feature-aware mod.rs for responses directory
pub fn generate_responses_mod_rs(
    output_dir: &Path,
    methods: &[RpcDef],
) -> Result<(), PipelineError> {
    let groups = group_methods_by_category(methods);

    let mut content = String::new();
    content.push_str("//! Response types organized by category\n");
    content.push_str("//!\n");
    content.push_str("//! This module contains response types for RPC methods,\n");
    content.push_str("//! organized by semantic category with feature-gated compilation.\n\n");

    // Generate conditional module declarations
    let mut categories: Vec<MethodCategory> = groups.keys().cloned().collect();
    categories.sort_by_key(|c| c.display_name());
    for category in categories {
        let feature_name = category.feature_name();
        let dir_name = category.dir_name();
        content.push_str(&format!("#[cfg(feature = \"{}\")]\n", feature_name));
        content.push_str(&format!("pub mod {};\n", dir_name));
        content.push_str(&format!("#[cfg(feature = \"{}\")]\n", feature_name));
        content.push_str(&format!("pub use {}::*;\n\n", dir_name));
    }

    // Write to responses/mod.rs
    let responses_dir = output_dir.join("responses");
    std::fs::create_dir_all(&responses_dir)?;
    let mod_rs_path = responses_dir.join("mod.rs");
    let content = trim_trailing_whitespace(&content);
    std::fs::write(mod_rs_path, content)?;

    Ok(())
}

/// Generate individual category modules
pub fn generate_category_modules(
    output_dir: &Path,
    methods: &[RpcDef],
    implementation: Implementation,
) -> Result<(), PipelineError> {
    let groups = group_methods_by_category(methods);

    // Create type adapter for protocol-specific type mapping
    let type_adapter = implementation.create_type_adapter().unwrap_or_else(|_| {
        panic!("Type adapter not available for protocol: {}", implementation.as_str())
    });

    // Generate individual category modules
    let mut categories: Vec<MethodCategory> = groups.keys().cloned().collect();
    categories.sort_by_key(|c| c.display_name());
    for category in categories {
        let feature_name = category.feature_name();
        let dir_name = category.dir_name();

        let mut content = String::new();
        content.push_str(&format!("//! {} RPC methods\n", category.dir_name()));
        content.push_str("//!\n");
        content.push_str(&format!(
            "//! This module contains RPC method implementations for {}.\n\n",
            category.dir_name()
        ));

        // Add feature gate
        content.push_str(&format!("#[cfg(feature = \"{}\")]\n", feature_name));
        content.push_str("use crate::responses::*;\n");
        content.push_str("use crate::types::*;\n\n");

        // Generate method implementations using MethodTemplate
        for method in groups.get(&category).into_iter().flat_map(|v| v.iter()) {
            let method_template = MethodTemplate::new(method, type_adapter.as_ref());
            let method_impl = method_template.render();
            content.push_str(&method_impl);
            content.push('\n');
        }

        // Write to file
        let methods_dir = output_dir.join("methods");
        std::fs::create_dir_all(&methods_dir)?;
        let module_path = methods_dir.join(format!("{}.rs", dir_name));
        let content = trim_trailing_whitespace(&content);
        std::fs::write(module_path, content)?;
    }

    Ok(())
}
