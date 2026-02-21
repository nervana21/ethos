//! Doc comment formatting for generated Rust code.
//!
//! All text that ends up in `///` doc lines must go through [sanitize_doc_line] so that
//! rustdoc sees escaped brackets and proper URL links (e.g. placeholders like `<txid>` and
//! bare URLs are handled correctly).

use ir::RpcDef;
use regex::Regex;

use crate::utils::rpc_method_to_rust_name;

/// Sanitize a line for use in Rust doc comments
///
/// Handles rustdoc-specific escaping:
/// - Wraps bare URLs in angle brackets for proper linking
/// - Escapes square brackets to prevent link interpretation
/// - Escapes angle brackets (except around URLs) to prevent HTML interpretation
pub fn sanitize_doc_line(line: &str) -> String {
    // First, wrap bare URLs in angle brackets for proper rustdoc linking
    let line = wrap_bare_urls(line);

    let mut result = String::new();
    let mut chars = line.chars().peekable();
    let mut in_url_link = false; // Track if we're inside <https://...>

    while let Some(ch) = chars.next() {
        match ch {
            '[' => {
                // Escape square brackets to prevent rustdoc from interpreting them as links
                result.push('\\');
                result.push('[');
            }
            ']' => {
                // Escape square brackets to prevent rustdoc from interpreting them as links
                result.push('\\');
                result.push(']');
            }
            '<' => {
                // Check if this is a URL wrapped in angle brackets (we want to keep those)
                // or an HTML-like tag that should be escaped
                let remaining: String = chars.clone().collect();
                if remaining.starts_with("http://") || remaining.starts_with("https://") {
                    // This is a URL link, keep the angle bracket and mark we're in a URL
                    in_url_link = true;
                    result.push('<');
                } else {
                    // Use HTML entity to prevent rustdoc from interpreting it as HTML
                    result.push_str("&lt;");
                }
            }
            '>' => {
                if in_url_link {
                    // We're closing a URL link, keep the bracket
                    in_url_link = false;
                    result.push('>');
                } else {
                    // Use HTML entity
                    result.push_str("&gt;");
                }
            }
            _ => result.push(ch),
        }
    }

    result
}

/// Wrap bare URLs in angle brackets for proper rustdoc linking
fn wrap_bare_urls(line: &str) -> String {
    // Match URLs - we'll check context manually since regex crate doesn't support look-around
    let url_pattern = Regex::new(r"https?://[^\s\)<>\]]+").expect("Invalid regex pattern");

    let mut result = String::new();
    let mut last_end = 0;

    for mat in url_pattern.find_iter(line) {
        let start = mat.start();
        let end = mat.end();

        // Add text before the URL
        result.push_str(&line[last_end..start]);

        // Check if URL is already wrapped in angle brackets (start/end are byte indices)
        let char_before = if start > 0 { line[..start].chars().last() } else { None };
        let char_after = line[end..].chars().next();

        if char_before == Some('<') && char_after == Some('>') {
            // Already wrapped, keep as-is
            result.push_str(mat.as_str());
        } else if char_before == Some('<') {
            // Has opening bracket but no closing - keep as-is (will be handled by angle bracket escaping)
            result.push_str(mat.as_str());
        } else {
            // Not wrapped, add angle brackets
            result.push('<');
            result.push_str(mat.as_str());
            result.push('>');
        }

        last_end = end;
    }

    // Add remaining text
    result.push_str(&line[last_end..]);

    result
}

/// Format documentation comments
pub fn format_doc_comment(description: &str) -> String {
    let mut doc = String::new();
    let mut current_section = String::new();
    let mut in_section = false;
    let mut first_section = true;
    let mut in_code_block = false;

    for line in description.lines() {
        let line = line.trim();

        // Handle code block markers
        if line.starts_with("```") {
            if !in_code_block {
                // Start of code block
                if !current_section.is_empty() {
                    // Process any pending section content
                    process_section(&mut doc, &current_section, in_section, &mut first_section);
                    current_section.clear();
                }
            }
            doc.push_str(&format!("/// {line}\n"));
            in_code_block = !in_code_block;
            continue;
        }

        // Process the line (sanitize only outside code blocks so e.g. ``` is not escaped)
        let processed_line = if in_code_block { line.to_string() } else { sanitize_doc_line(line) };

        if processed_line.is_empty() {
            if !current_section.is_empty() {
                process_section(&mut doc, &current_section, in_section, &mut first_section);
                current_section.clear();
            }
            in_section = false;
            // Don't add empty lines to avoid clippy warnings
            // doc.push_str("///\n");
        } else {
            if processed_line.starts_with("Arguments:")
                || processed_line.starts_with("Result:")
                || processed_line.starts_with("Examples:")
            {
                in_section = true;
                current_section.clear();
            }
            current_section.push_str(&processed_line);
            current_section.push('\n');
        }
    }

    // Process the last section
    if !current_section.is_empty() {
        process_section(&mut doc, &current_section, in_section, &mut first_section);
    }

    doc.trim_end().to_string()
}

fn process_section(doc: &mut String, section: &str, in_section: bool, first_section: &mut bool) {
    if !*first_section {
        // Don't add empty /// lines - this causes clippy warnings
        // doc.push_str("///\n");
    }
    *first_section = false;

    if section.starts_with("Arguments:") {
        doc.push_str("/// # Arguments\n");
        for section_line in section.lines().skip(1) {
            let section_line = section_line.trim();
            if !section_line.is_empty() {
                doc.push_str(&format!("/// {section_line}\n"));
            }
        }
    } else if section.starts_with("Result:") {
        doc.push_str("/// # Returns\n");
        for section_line in section.lines().skip(1) {
            let section_line = section_line.trim();
            if !section_line.is_empty() {
                doc.push_str(&format!("/// {section_line}\n"));
            }
        }
    } else if section.starts_with("Examples:") {
        doc.push_str("/// # Examples\n");
        for section_line in section.lines().skip(1) {
            let section_line = section_line.trim();
            if !section_line.is_empty() {
                doc.push_str(&format!("/// {section_line}\n"));
            }
        }
    } else if !in_section {
        // This is the description section
        for desc_line in section.lines() {
            let desc_line = desc_line.trim();
            if !desc_line.is_empty() {
                doc.push_str(&format!("/// {desc_line}\n"));
            }
        }
    }
}

/// Format a struct field with documentation
pub fn format_struct_field(field_name: &str, field_type: &str, description: &str) -> String {
    let desc = format_doc_comment(description);
    if desc.is_empty() {
        format!("    pub {field_name}: {field_type},\n")
    } else {
        format!("{desc}\n    pub {field_name}: {field_type},\n")
    }
}

/// Generate example usage documentation for an RPC method
pub fn generate_example_docs(method: &RpcDef) -> String {
    let mut docs = String::new();

    if !method.description.trim().is_empty() {
        let formatted_desc = format_doc_comment(&method.description);
        if !formatted_desc.is_empty() {
            docs.push_str(&formatted_desc);
        }
    }

    // Convert method name to Rust function name
    let rust_method_name = rpc_method_to_rust_name(&method.name);

    // Add a simple usage note without executable code
    docs.push_str("\n///\n/// # Usage\n");
    docs.push_str("/// This method can be called using the high-level client interface:\n");
    docs.push_str(&format!("/// - `client.{}(...).await`\n", rust_method_name));
    docs.push_str("/// Or directly via the transport layer for advanced use cases:\n");
    docs.push_str(&format!(
        "/// - `transport::{}(&transport, ...).await`\n///\n",
        rust_method_name
    ));

    docs.trim_end().to_string()
}

/// Write a doc comment line with proper prefix (sanitized for rustdoc).
pub fn write_doc_line(buf: &mut String, text: &str, indent: &str) -> std::fmt::Result {
    use std::fmt::Write;
    writeln!(buf, "{}/// {}", indent, sanitize_doc_line(text))
}

/// Write a multi-line doc comment
pub fn write_doc_comment(buf: &mut String, text: &str, indent: &str) -> std::fmt::Result {
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            write_doc_line(buf, trimmed, indent)?;
        }
    }
    Ok(())
}
