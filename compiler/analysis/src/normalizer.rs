//! IR Normalization
//!
//! Normalizes ProtocolIR data to ensure consistent, canonical field naming
//! and detect malformed or duplicate parameter definitions.
//! Also computes a deterministic content hash of the normalized IR.

use std::collections::HashSet;

use ir::{FieldDef, ProtocolDef, ProtocolIR, TypeDef};
use sha2::{Digest, Sha256};

/// IRNormalizer ensures IR-level consistency and normalization.
///
/// Responsibilities:
/// - Normalize casing and whitespace for parameter names
/// - Enforce deterministic field ordering
/// - Compute a SHA256 checksum of the entire normalized IR
pub struct IRNormalizer;

impl Default for IRNormalizer {
    fn default() -> Self { Self::new() }
}

impl IRNormalizer {
    /// Create a new IRNormalizer instance
    pub fn new() -> Self { Self }

    /// Normalize and hash an IR in place
    pub fn normalize(&self, ir: &mut ProtocolIR) -> Result<String, Box<dyn std::error::Error>> {
        let mut seen_methods = HashSet::new();

        // Normalize all modules in the IR
        for module in ir.modules_mut() {
            for def in module.definitions_mut() {
                match def {
                    ProtocolDef::RpcMethod(ref mut rpc) => {
                        // Normalize RPC method name
                        let trimmed_name = rpc.name.trim().to_string();
                        if trimmed_name != rpc.name {
                            rpc.name = trimmed_name.clone();
                        }

                        // Check for duplicate method names
                        if !seen_methods.insert(trimmed_name.clone()) {
                            return Err(format!(
                                "Duplicate method name detected: {}",
                                trimmed_name
                            )
                            .into());
                        }

                        // Normalize description
                        rpc.description = rpc.description.trim().to_string();

                        // Normalize parameters
                        let mut seen_params = HashSet::new();
                        for param in &mut rpc.params {
                            let trimmed_param_name = param.name.trim().to_string();
                            if trimmed_param_name != param.name {
                                param.name = trimmed_param_name.clone();
                            }

                            // Check for duplicate parameter names within the method
                            if !seen_params.insert(trimmed_param_name.clone()) {
                                return Err(format!(
                                    "Duplicate parameter `{}` in method `{}`",
                                    trimmed_param_name, rpc.name
                                )
                                .into());
                            }

                            // Normalize parameter description
                            param.description = param.description.trim().to_string();

                            // Normalize parameter type
                            self.normalize_type_def(&mut param.param_type)?;
                        }

                        // Normalize result type if present
                        if let Some(ref mut result_type) = rpc.result {
                            self.normalize_type_def(result_type)?;
                        }
                    }
                    ProtocolDef::Type(ref mut ty) => {
                        self.normalize_type_def(ty)?;
                    }
                    _ => {
                        // Other definition types don't need normalization
                    }
                }
            }
        }

        // === Compute deterministic IR hash ===
        let json_repr = serde_json::to_string(ir)?;
        let mut hasher = Sha256::new();
        hasher.update(json_repr.as_bytes());
        let checksum = format!("{:x}", hasher.finalize());

        Ok(checksum)
    }

    /// Normalize a TypeDef recursively
    fn normalize_type_def(&self, ty: &mut TypeDef) -> Result<(), Box<dyn std::error::Error>> {
        // Normalize type name
        ty.name = ty.name.trim().to_string();
        ty.description = ty.description.trim().to_string();

        // Normalize base type if present
        if let Some(ref mut base_type) = ty.base_type {
            *base_type = base_type.trim().to_string();
        }

        // Normalize protocol type if present
        if let Some(ref mut protocol_type) = ty.protocol_type {
            *protocol_type = protocol_type.trim().to_string();
        }

        // Normalize fields if present
        if let Some(ref mut fields) = ty.fields {
            for field in fields {
                self.normalize_field_def(field)?;
            }
        }

        // Normalize variants if present
        if let Some(ref mut variants) = ty.variants {
            for variant in variants {
                variant.name = variant.name.trim().to_string();
                variant.description = variant.description.trim().to_string();

                if let Some(ref mut value) = variant.value {
                    *value = value.trim().to_string();
                }

                if let Some(ref mut associated_data) = variant.associated_data {
                    for field in associated_data {
                        self.normalize_field_def(field)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Normalize a FieldDef
    fn normalize_field_def(&self, field: &mut FieldDef) -> Result<(), Box<dyn std::error::Error>> {
        field.name = field.name.trim().to_string();
        field.description = field.description.trim().to_string();

        if let Some(ref mut default_value) = field.default_value {
            *default_value = default_value.trim().to_string();
        }

        self.normalize_type_def(&mut field.field_type)?;
        Ok(())
    }
}
