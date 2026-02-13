//! This crate contains the code generators for the Bitcoin RPC API.
//!
//! The generators are responsible for generating the code for the Bitcoin RPC API.

/// Sub-crate generates: **`doc_comment`**
///
/// Produces Rust-doc comments and Markdown "Example:" blocks.
/// Transforms each `Method` into triple-slash doc comments injected into generated files.
pub mod doc_comment;

pub mod client_trait;

pub mod node_manager;
pub use node_manager::NodeManagerGenerator;

pub mod test_node;

/// Version-specific response type generator
pub mod version_specific_response_type;
pub use version_specific_response_type::VersionSpecificResponseTypeGenerator;

/// Version transition helpers
pub mod version_transitions;
pub use version_transitions::VersionTransitionRegistry;

/// Version-specific client trait generator
pub mod version_specific_client_trait;
pub use version_specific_client_trait::VersionSpecificClientTraitGenerator;

/// Version-specific generator trait for extensible implementation support
pub mod versioned_generator;
pub use versioned_generator::VersionedTypeGenerator;

/// Bitcoin Core version-specific generator
pub mod bitcoin_core_versioned;
pub use bitcoin_core_versioned::BitcoinCoreVersionedGenerator;

/// Registry for version-specific generators
pub mod versioned_registry;
pub use versioned_registry::VersionedGeneratorRegistry;
