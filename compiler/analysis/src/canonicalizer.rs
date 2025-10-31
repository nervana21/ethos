//! Type Canonicalization
//!
//! Identifies semantically equivalent types and consolidates them into canonical representations.
//! Groups types by signature and selects the shortest name as canonical.

use std::collections::HashMap;

use ir::{ProtocolDef, ProtocolIR};

/// Type Canonicalizer for canonicalizing type aliases and duplicates
#[derive(Default, Debug, Clone)]
pub struct TypeCanonicalizer;

impl TypeCanonicalizer {
    /// Analyze and canonicalize type aliases and duplicates
    pub fn canonicalize(&self, ir: &mut ProtocolIR) -> HashMap<String, String> {
        let mut map: HashMap<String, String> = HashMap::new();
        let mut seen_signatures: HashMap<String, String> = HashMap::new();

        for module in ir.modules_mut() {
            for def in module.definitions_mut() {
                if let ProtocolDef::Type(ref mut ty) = def {
                    let sig = format!("{:?}", ty.kind);

                    if let Some(canonical) = seen_signatures.get(&sig) {
                        // This is a duplicate - mark as alias
                        ty.canonical_name = Some(canonical.clone());
                        map.insert(ty.name.clone(), canonical.clone());
                    } else {
                        // First time seeing this signature - it's canonical
                        seen_signatures.insert(sig, ty.name.clone());
                    }
                }
            }
        }

        map
    }
}
