//! Scanner module — extracts `go!` blocks and `#[verify_rust_output]` attributes from source.
//!
//! Uses source text scanning for exact content extraction with proper
//! brace-depth tracking, combined with `proc_macro2` validation for
//! structural accuracy.

use proc_macro2::TokenStream;
use std::path::Path;
use walkdir::WalkDir;

/// Configuration for scanning operations.
pub struct ScanConfig {
    /// Skip paths containing these components.
    pub skip_components: Vec<&'static str>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            skip_components: Vec::new(),
        }
    }
}

impl ScanConfig {
    /// Create a new config that skips paths containing any of the given components.
    pub fn with_skip_components(mut self, components: Vec<&'static str>) -> Self {
        self.skip_components = components;
        self
    }

    /// Check if a path should be skipped based on configured components.
    pub fn should_skip(&self, path: &Path) -> bool {
        self.skip_components
            .iter()
            .any(|&comp| path.components().any(|c| c.as_os_str() == comp))
    }
}

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
pub fn scan_path(path: &Path) -> Result<Vec<GoBlock>, String> {
    scan_path_with_config(path, &ScanConfig::default())
}

/// Scan a path (file or directory) for `go!` blocks with custom configuration.
pub fn scan_path_with_config(path: &Path, config: &ScanConfig) -> Result<Vec<GoBlock>, String> {
    let mut blocks = Vec::new();

    if path.is_file() {
        blocks.extend(scan_file(path)?);
    } else if path.is_dir() {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| {
                e.file_type().is_dir()
                    || e.path().extension().map_or(false, |ext| ext == "rs")
            })
        {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().is_file() {
                if config.should_skip(entry.path()) {
                    continue;
                }
                blocks.extend(scan_file(entry.path())?);
            }
        }
    }

    blocks.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));
    Ok(blocks)
}

/// Scan a path (file or directory) for `#[verify_rust_output]` attributes.
pub fn scan_verify(path: &Path) -> Result<Vec<VerifyBlock>, String> {
    scan_verify_with_config(path, &ScanConfig::default())
}

/// Scan a path (file or directory) for `#[verify_rust_output]` attributes with custom configuration.
pub fn scan_verify_with_config(path: &Path, config: &ScanConfig) -> Result<Vec<VerifyBlock>, String> {
    let mut blocks = Vec::new();

    if path.is_file() {
        blocks.extend(scan_file_verify(path)?);
    } else if path.is_dir() {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| {
                e.file_type().is_dir()
                    || e.path().extension().map_or(false, |ext| ext == "rs")
            })
        {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().is_file() {
                if config.should_skip(entry.path()) {
                    continue;
                }
                blocks.extend(scan_file_verify(entry.path())?);
            }
        }
    }

    blocks.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));
    Ok(blocks)
}

/// Scan a single file for `go!` blocks.
fn scan_file(path: &Path) -> Result<Vec<GoBlock>, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {}", path.display(), e))?;

    let file = path
        .to_str()
        .ok_or_else(|| format!("non-UTF8 path: {}", path.display()))?
        .to_string();

    let blocks = find_go_blocks_from_source(&source, &file);
    Ok(blocks)
}

/// Scan a single file for `#[verify_rust_output]` attributes.
fn scan_file_verify(path: &Path) -> Result<Vec<VerifyBlock>, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {}", path.display(), e))?;

    let file = path
        .to_str()
        .ok_or_else(|| format!("non-UTF8 path: {}", path.display()))?
        .to_string();

    let blocks = find_verify_attributes(&source, &file);
    Ok(blocks)
}

/// Find all `go!` blocks in source text.
///
/// This is the entry point for the CLI tool.
///
/// 1. Scans source text to find `go!` patterns
/// 2. Uses `proc_macro2` to validate brace matching is correct
/// 3. Extracts the raw Go code from the source (preserving formatting)
pub fn find_go_blocks_from_source(source: &str, file: &str) -> Vec<GoBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        // Find `go!` in this line
        let go_pos = match line.find("go!") {
            Some(pos) => pos,
            None => continue,
        };

        // Check that what follows `go!` is `{` (ignoring whitespace)
        let after_raw = &line[go_pos + 3..];
        let after = after_raw.trim();
        if !after.starts_with('{') {
            continue;
        }

        // Calculate byte offset of the opening `{`.
        // Use the raw slice (before trim) to get the correct offset.
        let open_brace_in_raw = after_raw.find('{').unwrap();
        let line_byte_start: usize = source
            .lines()
            .take(line_idx)
            .map(|l| l.len() + 1)
            .sum();
        let open_brace_offset = line_byte_start + go_pos + 3 + open_brace_in_raw;

        // Use proc_macro2 to validate the block's brace structure is valid
        let fragment = &source[open_brace_offset..];
        let valid = if let Ok(ts) = fragment.parse::<TokenStream>() {
            // Check the first token is a brace group (the outermost `{...}`)
            matches!(ts.clone().into_iter().next(),
                Some(proc_macro2::TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Brace)
        } else {
            false
        };

        if valid {
            // Extract content using source text brace counting
            if let Some(content) = extract_brace_block(source, open_brace_offset) {
                blocks.push(GoBlock {
                    file: file.to_string(),
                    line: line_idx + 1,
                    content,
                });
            }
        }
    }

    blocks
}

/// Extract the content between matching braces from source text, starting from `open_brace`.
/// The `open_brace` must point to the opening `{`.
fn extract_brace_block(source: &str, open_brace: usize) -> Option<String> {
    if source.as_bytes()[open_brace] != b'{' {
        return None;
    }

    let mut depth = 0;
    let mut i = open_brace;

    while i < source.len() {
        let ch = source.as_bytes()[i];
        if ch == b'{' {
            depth += 1;
        } else if ch == b'}' {
            depth -= 1;
            if depth == 0 {
                // Extract content between the braces (exclusive)
                let content = source[open_brace + 1..i].trim().to_string();
                return Some(content);
            }
        }
        i += 1;
    }

    // Unclosed braces
    None
}

/// Find all `#[verify_rust_output({ ... })]` attributes in source text.
fn find_verify_attributes(source: &str, file: &str) -> Vec<VerifyBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        // Skip commented-out lines
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("///") {
            continue;
        }

        // Look for `verify_rust_output` in attribute lines
        let verify_pos = match line.find("verify_rust_output") {
            Some(pos) => pos,
            None => continue,
        };

        // Must be preceded by `[` (attribute syntax)
        let before = line[..verify_pos].trim_end();
        if !before.ends_with('[') {
            continue;
        }

        // Find the opening `{` of the brace group
        let after_keyword = &line[verify_pos + "verify_rust_output".len()..];
        let brace_pos = match after_keyword.find('{') {
            Some(p) => p,
            None => continue,
        };

        // Extract the brace group content using brace-depth tracking
        let line_byte_start: usize = source
            .lines()
            .take(line_idx)
            .map(|l| l.len() + 1)
            .sum();
        let open_col = verify_pos + "verify_rust_output".len() + brace_pos;
        let open_brace_offset = line_byte_start + open_col;

        let mut content = String::new();
        let mut brace_depth = 0;
        let mut i = open_brace_offset;

        while i < source.len() {
            let ch = source.as_bytes()[i];
            if ch == b'{' {
                brace_depth += 1;
                if brace_depth > 1 {
                    content.push(ch as char);
                }
            } else if ch == b'}' {
                brace_depth -= 1;
                if brace_depth == 0 {
                    break;
                } else {
                    content.push(ch as char);
                }
            } else {
                content.push(ch as char);
            }
            i += 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_line_block() {
        let source = "go! { func hello() int { return 42 } }\n";
        let blocks = find_go_blocks_from_source(source, "test.rs");
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
        let blocks = find_go_blocks_from_source(source, "test.rs");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content, "func hello() int {\n        return 42\n    }");
        assert_eq!(blocks[0].line, 1);
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
        let blocks = find_go_blocks_from_source(source, "test.rs");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].content.contains("if true"));
    }

    #[test]
    fn test_multiple_blocks() {
        let source = "go! { func a() int { return 1 } }\ngo! { func b() int { return 2 } }\n";
        let blocks = find_go_blocks_from_source(source, "test.rs");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].content, "func a() int { return 1 }");
        assert_eq!(blocks[1].content, "func b() int { return 2 }");
    }

    #[test]
    fn test_verify_attribute() {
        let source = r#"#[verify_rust_output({
    fn go_add(n: i32) -> i32 {
        n + 1
    }
})]
go! {
    func goAdd(n int) int {
        n + 1
    }
}"#;
        let blocks = find_verify_attributes(source, "test.rs");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].content.contains("fn go_add"));
    }

    #[test]
    fn test_empty_verify() {
        let source = r#"#[verify_rust_output({})]
go! { func hello() int { return 42 } }"#;
        let blocks = find_verify_attributes(source, "test.rs");
        // Empty verify block is still a valid verify attribute
        assert!(blocks.len() >= 1);
    }
}
