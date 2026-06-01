//! Extract `go!` blocks from Rust source files.
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

/// Find all `go!` blocks in source text using brace matching.
fn find_go_blocks(source: &str, file: &str) -> Vec<GoBlock> {
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
