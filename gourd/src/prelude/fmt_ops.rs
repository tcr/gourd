//! Go's `fmt` package helpers.
//!
//! Provides `Sprintf`, `Print`, `Println`, and `Printf` format functions.

/// Go's `fmt.Sprintf` — formatted string output.
///
/// Supports simple format specifiers: `%d` (int), `%s` (string),
/// `%v` (value), `%f` (float).
pub fn fmt_sprintf(format: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut result = String::new();
    let mut args_iter = args.iter();

    for c in format.chars() {
        if c == '%' {
            match format.chars().nth(format.find(c).map(|i| i + 1).unwrap_or(format.len())) {
                Some('d' | 's' | 'v' | 'f') => {
                    if let Some(arg) = args_iter.next() {
                        result.push_str(&format!("{}", arg));
                    }
                }
                Some(unknown) => {
                    result.push('%');
                    result.push(unknown);
                }
                None => {
                    result.push('%');
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Go's `fmt.Print` — formatted string output to stdout.
pub fn fmt_print(format: &str, args: &[&dyn std::fmt::Display]) {
    let result = fmt_sprintf(format, args);
    println!("{}", result);
}

/// Go's `fmt.Println` — formatted string output to stdout with newline.
pub fn fmt_println(format: &str, args: &[&dyn std::fmt::Display]) {
    let result = fmt_sprintf(format, args);
    println!("{}", result);
}

/// Go's `fmt.Printf` — formatted string output to stdout (no trailing newline).
pub fn fmt_printf(format: &str, args: &[&dyn std::fmt::Display]) {
    let result = fmt_sprintf(format, args);
    print!("{}", result);
}
