// SPDX-License-Identifier: CC0-1.0

//! Binary entry point for the Bitcoin Core schema processor.
//!
//! Delegates to the library implementation so that the schema module is only
//! compiled as part of the lib (where `crate::convert_helpers` resolves).

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Err(e) = ethos_adapters::bitcoin_core::schema::run(&args) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
