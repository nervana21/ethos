//! Generated Bitcoin Core **Raw** RPC response types (same output as production codegen).
//!
//! Used only to keep **serde decode** goldens in sync with the generator and IR.

#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(unused_imports)]

mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated_responses.rs"));
}

pub use generated::*;
