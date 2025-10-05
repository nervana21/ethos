// SPDX-License-Identifier: CC0-1.0

//! Ethos umbrella crate.
//!
//! This crate primarily serves as the workspace root.
//!
//! All functional code lives in the workspace member crates under
//! directories such as `adapters`, `compiler`, and others.

#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]
#![warn(deprecated_in_future)]
#![doc(test(attr(warn(unused))))]

/// Miscellaneous metadata about the Ethos workspace.
pub mod ethos_meta {
    /// Version string for the umbrella crate, as reported by Cargo.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
