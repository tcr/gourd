//! Format validation results as human-readable output.

use crate::validator::{CheckResult, Validation};

pub fn format_results(results: &[CheckResult]) -> String {
    let mut output = String::new();

    for r in results {
        if let Some(ref v) = r.go_valid {
            if let Validation::Error(msg) = v {
                let code_lines: Vec<String> = r
                    .go_code
                    .lines()
                    .enumerate()
                    .map(|(i, line)| format!("{} | {}", i + 1, line))
                    .collect();
                output.push_str(&format!(
                    "  {}:{}\n    Go: {}\n    {}\n\n",
                    r.file,
                    r.line,
                    colorize_error(msg),
                    code_lines.join("\n")
                ));
            }
        }

        if let Some(ref v) = r.rust_valid {
            if let Validation::Error(msg) = v {
                output.push_str(&format!(
                    "  {}:{}\n    Rust: {}\n\n",
                    r.file,
                    r.line,
                    colorize_error(msg)
                ));
            }
        }
    }

    if output.is_empty() {
        "  ✓ All blocks valid\n".to_string()
    } else {
        format!("\n{}errors found\n", output)
    }
}

fn colorize_error(msg: &str) -> String {
    format!("\x1b[31m{}\x1b[0m", msg)
}
