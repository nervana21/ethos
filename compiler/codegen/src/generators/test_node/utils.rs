//! Utility functions for test node generation

/// Capitalizes the first character of a string and converts snake_case/kebab-case to PascalCase.
///
/// This function takes a string and converts it to PascalCase by:
/// - Capitalizing the first character
/// - Converting underscores and hyphens to spaces
/// - Capitalizing the first letter of each word
/// - Removing spaces and converting to uppercase
pub fn camel(s: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            up = true;
        } else if up {
            out.push(ch.to_ascii_uppercase());
            up = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Generates a module file for the protocol-specific test node client.
///
/// This function creates a module file that contains the client structs and implementations
/// for the test node client. The generated file is specific to the given protocol
/// (e.g. Bitcoin Core) but uses a protocol-agnostic design via
/// NodeManager dependency injection.
pub fn generate_mod_rs(implementation_display_name: &str, client_name: &str) -> String {
    format!(
        "//! {} test node client\n\
         //! \n\
         //! This module provides a test client for {} that works with any NodeManager\n\
         //! implementation via dependency injection.\n\
         pub mod params;\n\
         pub mod client;\n\n\
         // re-export common clients\n\
         pub use client::{};\n",
        implementation_display_name, implementation_display_name, client_name
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camel() {
        let result = camel("ab_c");
        assert_eq!(result, "AbC");
    }

    #[test]
    fn test_generate_mod_rs() {
        let result = generate_mod_rs("Bitcoin Core", "BitcoinTestClient");
        assert!(result.contains("//! Bitcoin Core test node client"));
        assert!(result.contains("for Bitcoin Core that works with any NodeManager"));
        assert!(result.contains("pub mod params;"));
        assert!(result.contains("pub mod client;"));
        assert!(result.contains("pub use client::BitcoinTestClient;"));
    }
}
