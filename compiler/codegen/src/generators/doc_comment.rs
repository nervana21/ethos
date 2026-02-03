use ir::RpcDef;

use crate::utils::rpc_method_to_rust_name;

/// Sanitize a line for use in Rust doc comments
pub fn sanitize_doc_line(line: &str) -> String {
    let mut result = String::new();
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '`' => {
                // Escape backticks by doubling them for markdown
                result.push('`');
                result.push('`');
            }
            '\'' => {
                // Handle single quotes that might be confused with character literals
                // Check if this looks like a character literal
                if let Some(&next_char) = chars.peek() {
                    if next_char != ' ' {
                        // This might be a character literal, escape it
                        result.push('\\');
                        result.push('\'');
                    } else {
                        // Regular apostrophe, convert to double quote
                        result.push('"');
                    }
                } else {
                    // Regular apostrophe, convert to double quote
                    result.push('"');
                }
            }
            '\\' => {
                // Escape backslashes
                result.push('\\');
                result.push('\\');
            }
            _ => result.push(ch),
        }
    }

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

        // Process the line
        let processed_line =
            if !in_code_block { sanitize_doc_line(line) } else { line.to_string() };

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
                let sanitized_line = sanitize_doc_line(section_line);
                doc.push_str(&format!("/// {sanitized_line}\n"));
            }
        }
    } else if section.starts_with("Result:") {
        doc.push_str("/// # Returns\n");
        for section_line in section.lines().skip(1) {
            let section_line = section_line.trim();
            if !section_line.is_empty() {
                let sanitized_line = sanitize_doc_line(section_line);
                doc.push_str(&format!("/// {sanitized_line}\n"));
            }
        }
    } else if section.starts_with("Examples:") {
        doc.push_str("/// # Examples\n");
        for section_line in section.lines().skip(1) {
            let section_line = section_line.trim();
            if !section_line.is_empty() {
                let sanitized_line = sanitize_doc_line(section_line);
                doc.push_str(&format!("/// {sanitized_line}\n"));
            }
        }
    } else if !in_section {
        // This is the description section
        for desc_line in section.lines() {
            let desc_line = desc_line.trim();
            if !desc_line.is_empty() {
                let sanitized_line = sanitize_doc_line(desc_line);
                doc.push_str(&format!("/// {sanitized_line}\n"));
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
    docs.push_str("\n/// # Usage\n");
    docs.push_str("/// This method can be called using the high-level client interface:\n");
    docs.push_str(&format!("/// - `client.{}(...).await`\n", rust_method_name));
    docs.push_str("/// Or directly via the transport layer for advanced use cases:\n");
    docs.push_str(&format!("/// - `transport::{}(&transport, ...).await`\n", rust_method_name));

    docs.trim_end().to_string()
}

/// Write a sanitized doc comment line with proper prefix
pub fn write_doc_line(buf: &mut String, text: &str, indent: &str) -> std::fmt::Result {
    use std::fmt::Write;
    let sanitized = sanitize_doc_line(text);
    writeln!(buf, "{}/// {}", indent, sanitized)
}

/// Write a sanitized multi-line doc comment
pub fn write_doc_comment(buf: &mut String, text: &str, indent: &str) -> std::fmt::Result {
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            write_doc_line(buf, trimmed, indent)?;
        }
    }
    Ok(())
}
