//! Version transition helpers for handling field renames, type changes, and deprecations
//!
//! This module provides utilities for managing the evolution of Bitcoin Core RPC types
//! across different versions, including field renames, type changes, and deprecation handling.

use std::collections::HashMap;

use types::ProtocolVersion;

/// Information about a field transition between versions
#[derive(Debug, Clone)]
pub struct FieldTransition {
    /// The field name in the source version
    pub from_name: String,
    /// The field name in the target version
    pub to_name: String,
    /// The version when the transition occurred
    pub transition_version: String,
    /// Whether this is a rename (true) or addition/removal (false)
    pub is_rename: bool,
}

/// Information about a type change between versions
#[derive(Debug, Clone)]
pub struct TypeTransition {
    /// The field name
    pub field_name: String,
    /// The old type
    pub from_type: String,
    /// The new type
    pub to_type: String,
    /// The version when the change occurred
    pub transition_version: String,
}

/// Information about a deprecation
#[derive(Debug, Clone)]
pub struct DeprecationInfo {
    /// The field name
    pub field_name: String,
    /// The version when the field was deprecated
    pub deprecated_in: String,
    /// The deprecation message
    pub message: Option<String>,
}

/// Registry for managing version transitions
#[derive(Default)]
pub struct VersionTransitionRegistry {
    /// Field transitions by struct name
    field_transitions: HashMap<String, Vec<FieldTransition>>,
    /// Type transitions by struct name
    type_transitions: HashMap<String, Vec<TypeTransition>>,
    /// Deprecation information by struct name
    deprecations: HashMap<String, Vec<DeprecationInfo>>,
}

impl VersionTransitionRegistry {
    /// Create a new version transition registry
    pub fn new() -> Self {
        Self {
            field_transitions: HashMap::new(),
            type_transitions: HashMap::new(),
            deprecations: HashMap::new(),
        }
    }

    /// Analyze field transitions for a struct across versions
    /// Simplified - IR doesn't track per-type versions, so this is a no-op
    pub fn analyze_struct_transitions(
        &mut self,
        _struct_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // IR doesn't track per-type versions, so transitions can't be analyzed
        Ok(())
    }

    /// Get field transitions for a struct
    pub fn get_field_transitions(&self, struct_name: &str) -> Option<&Vec<FieldTransition>> {
        self.field_transitions.get(struct_name)
    }

    /// Get type transitions for a struct
    pub fn get_type_transitions(&self, struct_name: &str) -> Option<&Vec<TypeTransition>> {
        self.type_transitions.get(struct_name)
    }

    /// Get deprecation information for a struct
    pub fn get_deprecations(&self, struct_name: &str) -> Option<&Vec<DeprecationInfo>> {
        self.deprecations.get(struct_name)
    }

    /// Generate version-specific field documentation
    pub fn generate_field_doc(
        &self,
        struct_name: &str,
        field_name: &str,
        version: &ProtocolVersion,
    ) -> String {
        let mut doc_parts = Vec::new();

        // Check if field is deprecated in this version
        if let Some(deprecations) = self.deprecations.get(struct_name) {
            if let Some(deprecation) = deprecations.iter().find(|d| d.field_name == field_name) {
                let version_num = parse_version_number(version.as_str()).unwrap_or(0);
                let deprecation_num = parse_version_number(&deprecation.deprecated_in).unwrap_or(0);

                if version_num >= deprecation_num {
                    doc_parts.push(format!(
                        "**DEPRECATED** since {}: {}",
                        deprecation.deprecated_in,
                        deprecation.message.as_deref().unwrap_or("This field is deprecated")
                    ));
                }
            }
        }

        // Check for field renames
        if let Some(transitions) = self.field_transitions.get(struct_name) {
            if let Some(transition) =
                transitions.iter().find(|t| t.to_name == field_name && t.is_rename)
            {
                doc_parts.push(format!(
                    "Renamed from `{}` in {}",
                    transition.from_name, transition.transition_version
                ));
            }
        }

        // Check for type changes
        if let Some(transitions) = self.type_transitions.get(struct_name) {
            if let Some(transition) = transitions.iter().find(|t| t.field_name == field_name) {
                doc_parts.push(format!(
                    "Type changed from `{}` to `{}` in {}",
                    transition.from_type, transition.to_type, transition.transition_version
                ));
            }
        }

        if doc_parts.is_empty() {
            format!("/// {}", field_name)
        } else {
            format!("/// {}\n/// {}", field_name, doc_parts.join("\n/// "))
        }
    }

    /// Generate version-specific struct documentation
    pub fn generate_struct_doc(&self, struct_name: &str, version: &ProtocolVersion) -> String {
        let mut doc_parts = Vec::new();

        // Add version information
        doc_parts.push(format!("Response type for Bitcoin Core {}", version.as_str()));

        // Add deprecation warnings
        if let Some(deprecations) = self.deprecations.get(struct_name) {
            let version_num = parse_version_number(version.as_str()).unwrap_or(0);
            let relevant_deprecations: Vec<_> = deprecations
                .iter()
                .filter(|d| {
                    let dep_version_num = parse_version_number(&d.deprecated_in).unwrap_or(0);
                    version_num >= dep_version_num
                })
                .collect();

            if !relevant_deprecations.is_empty() {
                doc_parts.push("**Note**: This version contains deprecated fields.".to_string());
            }
        }

        format!("/// {}\n///\n/// {}", struct_name, doc_parts.join("\n/// "))
    }
}

/// Parse a version string into a numeric value for comparison
fn parse_version_number(version_str: &str) -> Option<i32> {
    let clean_version = version_str.trim_start_matches('v');
    let parts: Vec<&str> = clean_version.split('.').collect();

    if parts.is_empty() {
        return None;
    }

    parts[0].parse::<i32>().ok()
}
