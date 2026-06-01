//! Extract `go!` blocks and `#[verify_rust_output]` attributes from Rust source files.
//!
//! Uses brace-matching to find the content inside `go!` patterns,
//! preserving the exact source text including formatting.

use anyhow::{Context, Result};
use std::path::Path;
use walkdir::WalkDir;

/// A discovered Go block with its source location.
#[derive(Debug, Clone)]
pub struct GoBlock {
    /// The source file path containing this block.
    pub file: String,
    /// The line number where `go!` starts (1-indexed).
    pub line: usize,
    /// The raw Go source text inside the `go!` braces.
    pub content: String,
}

/// A discovered `#[verify_rust_output]` attribute with its source location.
/// The `content` contains the expected Rust tokens from the brace group.
#[derive(Debug, Clone)]
pub struct VerifyBlock {
    /// The source file path containing this attribute.
    pub file: String,
    /// The line number where `#[verify_rust_output]` starts (1-indexed).
    pub line: usize,
    /// The expected Rust source text inside the verify brace group.
    pub content: String,
}

/// Scan a path (file or directory) for `go!` blocks.
pub fn scan_path(path: &Path) -> Result<Vec<GoBlock>> {
    let mut blocks = Vec::new();

    if path.is_file() {
        blocks.extend(scan_file(path)?);
    } else if path.is_dir() {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| {
                e.file_type().is_dir() || {
                    e.path().extension().map_or(false, |ext| ext == "rs")
                }
            })
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                blocks.extend(scan_file(entry.path())?);
            }
        }
    }

    blocks.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));
    Ok(blocks)
}

/// Scan a path (file or directory) for `#[verify_rust_output]` attributes.
pub fn scan_verify(path: &Path) -> Result<Vec<VerifyBlock>> {
    let mut blocks = Vec::new();

    if path.is_file() {
        blocks.extend(scan_file_verify(path)?);
    } else if path.is_dir() {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| {
                e.file_type().is_dir() || {
                    e.path().extension().map_or(false, |ext| ext == "rs")
                }
            })
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                blocks.extend(scan_file_verify(entry.path())?);
            }
        }
    }

    blocks.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));
    Ok(blocks)
}

/// Scan a single file for `go!` blocks.
fn scan_file(path: &Path) -> Result<Vec<GoBlock>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let file = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF8 path: {}", path.display()))?
        .to_string();

    let blocks = find_go_blocks(&source, &file);
    Ok(blocks)
}

/// Scan a single file for `#[verify_rust_output]` attributes.
fn scan_file_verify(path: &Path) -> Result<Vec<VerifyBlock>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let file = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF8 path: {}", path.display()))?
        .to_string();

    let blocks = find_verify_attributes(&source, &file);
    Ok(blocks)
}

/// Find all `#[verify_rust_output]` attributes in source text.
/// Extracts the expected Rust code from the brace group.
fn find_verify_attributes(source: &str, file: &str) -> Vec<VerifyBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        // Skip commented-out lines (// or /* ... */)
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("///") {
            continue;
        }

        // Look for `verify_rust_output` in attribute lines
        let verify_pos = match line.find("verify_rust_output") {
            Some(pos) => pos,
            None => continue,
        };

        // Make sure this is actually an attribute (starts with `#[` or `[#`)
        let before = line[..verify_pos].trim_end();
        if !before.ends_with('[') {
            continue;
        }

        // Find the opening `{` of the brace group (short form: `[{...}]` or longer form: `[{verify = {...}}]`)
        // Search for `{` after `verify_rust_output`
        let after_keyword = &line[verify_pos + "verify_rust_output".len()..];
        let brace_pos = match after_keyword.find('{') {
            Some(p) => p,
            None => continue,
        };

        // Found the opening brace — extract the brace group content.
        // The first '{' sets depth to 1; don't include it in content.
        let open_col = verify_pos + "verify_rust_output".len() + brace_pos;
        let mut content = String::new();
        let mut brace_depth = 0;

        // First '{' sets brace_depth to 1 (outer delimiter)
        brace_depth += 1;
        // Continue from the character AFTER the opening '{'
        for col in (open_col + 1)..line.len() {
            let ch = line[col..].chars().next().unwrap();
            if ch == '{' {
                brace_depth += 1;
            } else if ch == '}' {
                brace_depth -= 1;
                if brace_depth == 0 {
                    break;
                }
            }
            content.push(ch);
        }

        // Continue across multiple lines if brace group spans lines
        let mut line_num = line_idx + 1;
        while brace_depth > 0 && line_num < lines.len() {
            let line = lines[line_num];
            for ch in line.chars() {
                if ch == '{' {
                    brace_depth += 1;
                    content.push(ch);
                } else if ch == '}' {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        break;
                    } else {
                        content.push(ch);
                    }
                } else {
                    content.push(ch);
                }
            }
            if brace_depth > 0 {
                content.push('\n');
            }
            line_num += 1;
        }

        if brace_depth == 0 {
            blocks.push(VerifyBlock {
                file: file.to_string(),
                line: line_idx + 1,
                content: content.trim().to_string(),
            });
        }
    }

    blocks
}

/// Find all `go!` blocks in source text using brace matching.
///
/// This function operates on raw source text and is suitable for CLI usage
/// where you want to extract Go code without going through the scanner.
pub fn find_go_blocks(source: &str, file: &str) -> Vec<GoBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        let go_pos = match line.find("go!") {
            Some(pos) => pos,
            None => continue,
        };

        // Check that what follows `go!` is `{` (ignoring whitespace)
        let after = line[go_pos + 3..].trim();
        if !after.starts_with('{') {
            continue;
        }

        let open_offset = line[go_pos + 3..].find('{').unwrap();
        let open_col = go_pos + 3 + open_offset;

        let mut content = String::new();
        let mut brace_depth = 0;
        let mut in_block = false;

        for col in open_col..line.len() {
            let ch = line[col..].chars().next().unwrap();
            if ch == '{' {
                brace_depth += 1;
                in_block = true;
            } else if ch == '}' {
                brace_depth -= 1;
                if brace_depth == 0 {
                    break;
                }
            }
            if in_block {
                content.push(ch);
            }
        }

        let mut line_num = line_idx + 1;
        while brace_depth > 0 && line_num < lines.len() {
            let line = lines[line_num];
            for ch in line.chars() {
                if ch == '{' {
                    brace_depth += 1;
                } else if ch == '}' {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        break;
                    }
                }
                content.push(ch);
            }
            if brace_depth > 0 {
                content.push('\n');
            }
            line_num += 1;
        }

        if brace_depth == 0 {
            // Strip the surrounding braces that were included in content.
            // The first char is '{' and the last is '}'.
            let trimmed = content
                .trim_start_matches('{')
                .trim_end_matches('}');
            blocks.push(GoBlock {
                file: file.to_string(),
                line: line_idx + 1,
                content: trimmed.trim().to_string(),
            });
        }
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_line_block() {
        let source = "go! { func hello() int { return 42 } }\n";
        let blocks = find_go_blocks(source, "test.rs");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content, "func hello() int { return 42 }");
        assert_eq!(blocks[0].line, 1);
    }

    #[test]
    fn test_multiline_block() {
        let source = r#"go! {
    func hello() int {
        return 42
    }
}"#;
        let blocks = find_go_blocks(source, "test.rs");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content, "func hello() int {\n        return 42\n    }");
    }

    #[test]
    fn test_nested_braces() {
        let source = r#"go! {
    func foo() int {
        if true {
            return 1
        }
        return 0
    }
}"#;
        let blocks = find_go_blocks(source, "test.rs");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].content.contains("if true"));
    }

    #[test]
    fn test_multiple_blocks() {
        let source = "go! { func a() int { return 1 } }\ngo! { func b() int { return 2 } }\n";
        let blocks = find_go_blocks(source, "test.rs");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].content, "func a() int { return 1 }");
        assert_eq!(blocks[1].content, "func b() int { return 2 }");
    }
}
