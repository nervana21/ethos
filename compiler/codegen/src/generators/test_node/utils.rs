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

/// Generates a module file for the protocol-agnostic test node client.
///
/// This function creates a module file that contains the client structs and implementations
/// for the test node client. The generated file is protocol-agnostic and works with any
/// NodeManager implementation via dependency injection.
pub fn generate_mod_rs(_implementation: &str, client_name: &str) -> String {
    format!(
        "//! Protocol-agnostic test node module\n\
         //! \n\
         //! This module provides a generic test client that works with any NodeManager\n\
         //! implementation via dependency injection. Use the appropriate node manager\n\
         //! for your specific protocol (Bitcoin Core, Core Lightning, etc.).\n\
         pub mod params;\n\
         pub mod client;\n\n\
         // re-export common clients\n\
         pub use client::{};\n",
        client_name
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
        let result = generate_mod_rs("implementation", "TestClient");
        assert!(result.contains("//! Protocol-agnostic test node module"));
        assert!(result.contains("pub mod params;"));
        assert!(result.contains("pub mod client;"));
        assert!(result.contains("pub use client::TestClient;"));
    }
}
