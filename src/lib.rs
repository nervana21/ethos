//! Root crate stub to satisfy Cargo when invoking commands like `cargo clean`.
//! All functional code lives inside workspace members under `./adapters`, `./compiler`, etc.

#![allow(dead_code)]

/// Placeholder module to keep the root package compilable.
pub mod ethos_meta {
    /// Version string for the umbrella crate.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
