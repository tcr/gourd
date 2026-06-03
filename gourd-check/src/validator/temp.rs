//! Temporary directory functions.
//!
//! Provides functions for running `go build` and `cargo check` on
//! temporary directories.

use std::process::Command;
use std::io;
use std::path::Path;

/// Run `go build` on a temporary directory.
pub fn run_go_build(dir: &Path, code: &str) -> io::Result<()> {
    let main_go = dir.join("main.go");
    // Extract struct/interface definitions and place them before main().
    // Go requires type declarations to appear before they are used.
    let (structs, funcs) = separate_struct_defs(code);
    let wrapped = format!(
        "package main\n\nimport (\n    \"fmt\"\n    \"unsafe\"\n)\n\nvar _ = fmt.Print\nvar _ = unsafe.Sizeof(0)\n\n{structs}\n\nfunc main() {{}}\n\n{funcs}\n",
    );
    std::fs::write(&main_go, &wrapped)?;

    let output = Command::new("go")
        .args(["build", "-o", "/dev/null"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(io::Error::new(io::ErrorKind::Other, stderr.trim().to_string()))
    }
}

/// Separate struct/interface declarations from function declarations.
/// Returns (struct_code, func_code) — structs go first, funcs second.
fn separate_struct_defs(code: &str) -> (String, String) {
    let mut structs = Vec::new();
    let mut funcs = Vec::new();
    let mut in_struct = false;
    let mut block_depth = 0;
    let mut current_buf = String::new();

    for line in code.split('\n') {
        let trimmed = line.trim_start();

        if !in_struct && block_depth == 0 {
            // Check if this line starts a struct/interface definition
            if trimmed.starts_with("struct ")
                || trimmed.starts_with("struct{")
                || trimmed.starts_with("interface ")
                || trimmed.starts_with("interface{")
            {
                in_struct = true;
                block_depth = trimmed.matches('{').count() - trimmed.matches('}').count();
                current_buf.push_str(line);
                current_buf.push('\n');
                continue;
            }
        }

        if in_struct {
            block_depth += trimmed.matches('{').count() - trimmed.matches('}').count();
            current_buf.push_str(line);
            current_buf.push('\n');
            if block_depth <= 0 {
                structs.push(current_buf.trim_end().to_string());
                current_buf.clear();
                in_struct = false;
                block_depth = 0;
            }
            continue;
        }

        // Outside struct definitions — these are functions
        funcs.push(line.to_string());
    }

    (structs.join("\n\n"), funcs.join("\n"))
}

/// Run `cargo check` on a temporary directory.
pub fn run_cargo_check(dir: &Path, code: &str) -> io::Result<()> {
    let main_rs = dir.join("src").join("main.rs");
    std::fs::write(&main_rs, code)?;

    let output = Command::new("cargo")
        .args(["check", "-q"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(io::Error::new(io::ErrorKind::Other, stderr.trim().to_string()))
    }
}
