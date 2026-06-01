//! Format validation results as human-readable output.

use crate::validator::{FormatResult, Validation};

pub fn format_results(results: &[FormatResult]) -> String {
    let mut output = String::new();

    for r in results {
        if let Some(ref v) = r.validation {
            if let Validation::Error(msg) = v {
                let code_lines: Vec<String> = r
                    .content
                    .lines()
                    .enumerate()
                    .map(|(i, line)| format!("{} | {}", i + 1, line))
                    .collect();
                output.push_str(&format!(
                    "  {}:{}\n    {}\n    {}\n\n",
                    r.file,
                    r.line,
                    colorize_error(msg),
                    code_lines.join("\n")
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
