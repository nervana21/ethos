// SPDX-License-Identifier: CC0-1.0

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Path utility functions for finding project roots and resolving paths.
//!
//! This module provides utilities for finding project roots, validating input paths,
//! loading registries, and resolving protocol specification paths.

pub mod path_utils;

// Re-export for convenience
pub use path_utils::*;
