//! Code normalization functions.
//!
//! Converts Go dialect code to valid Go or Rust code for validation.

/// Convert CheckResults to FormatResults.
pub fn check_results_to_format(results: Vec<super::types::CheckResult>) -> Vec<super::types::FormatResult> {
    results
        .into_iter()
        .map(|r| super::types::FormatResult {
            file: r.file,
            line: r.line,
            content: r.go_code,
            validation: r.go_valid,
        })
        .collect()
}

/// Convert VerifyChecks to FormatResults.
pub fn verify_checks_to_format(results: Vec<super::types::VerifyCheck>) -> Vec<super::types::FormatResult> {
    results
        .into_iter()
        .map(|r| super::types::FormatResult {
            file: r.file,
            line: r.line,
            content: r.rust_code,
            validation: r.validation,
        })
        .collect()
}

/// Normalize Go dialect code for validation by the real Go compiler.
/// Converts:
/// - `while` loops → `for` loops (Go doesn't have `while`)
/// - `struct Name { ... }` → `type Name struct { ... }`
pub fn normalize_go_code(code: &str) -> String {
    // Step 1: `while` → `for`
    let step1 = normalize_while(code);
    // Step 2: `struct` → `type Name struct`
    normalize_struct(&step1)
}

/// Replace `while` with `for` at word boundaries.
fn normalize_while(code: &str) -> String {
    let mut result = String::with_capacity(code.len());
    let mut pos = 0;

    while pos < code.len() {
        if code[pos..].starts_with("while") {
            let after_end = pos + 5;
            let prev_is_ident = pos > 0 && {
                let prev: char = code.chars().nth(pos - 1).unwrap_or(' ');
                prev.is_alphanumeric() || prev == '_'
            };
            let after_is_ident = after_end < code.len() && {
                let after: char = code.chars().nth(after_end).unwrap_or(' ');
                after.is_alphanumeric() || after == '_'
            };
            if !prev_is_ident && !after_is_ident {
                result.push_str("for");
                pos += 5;
                continue;
            }
        }
        result.push(code.chars().nth(pos).unwrap());
        pos += 1;
    }
    result
}

/// Replace `struct Name { ... }` → `type Name struct { ... }`.
fn normalize_struct(code: &str) -> String {
    let mut result = String::new();
    let mut in_block = false;
    let mut block_depth = 0;

    for line in code.split('\n') {
        let trimmed = line.trim_start();

        // Track if we're inside a struct block
        if !in_block {
            if trimmed.starts_with("struct ") || trimmed.starts_with("struct{") {
                // This is a struct declaration line
                let indent: String = line.chars().take(line.len() - trimmed.len()).collect();
                let without_struct = &trimmed[7..]; // skip "struct "
                let name_end = without_struct.find(|c: char| c == '{' || c == '(')
                    .unwrap_or(without_struct.len());
                let name = without_struct[..name_end].trim();
                let rest = &without_struct[name_end..]; // "{ ... }" or "(...) { ... }"

                result.push_str(&indent);
                result.push_str(&format!("type {} struct{}", name, rest));
                in_block = true;
                block_depth = rest.matches('{').count();
                continue;
            }
        } else {
            // Inside a struct block, track brace depth
            for ch in line.chars() {
                if ch == '{' { block_depth += 1; }
                if ch == '}' { block_depth -= 1; }
            }
            result.push_str(line);
            if block_depth == 0 {
                in_block = false;
            }
        }

        result.push('\n');
    }

    // Remove trailing newline
    result.trim_end().to_string()
}
