//! Go's `fmt` package helpers.
//!
//! Provides `Sprintf`, `Print`, `Println`, and `Printf` format functions.

use crate::GoString;

/// Go's `fmt.Sprintf` — formatted string output.
///
/// Supports simple format specifiers: `%d` (int), `%s` (string),
/// `%v` (value), `%f` (float).
pub fn fmt_sprintf<F: AsRef<str>, A: IntoIterator>(format: F, args: A) -> GoString
where
    A::Item: std::fmt::Display,
{
    let format_str = format.as_ref();
    let args_display: Vec<String> = args.into_iter().map(|s| format!("{}", s)).collect();
    let mut result = String::new();
    let mut args_iter = args_display.iter();

    let mut chars = format_str.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            // Skip any width/padding modifiers (digits, 0, -)
            // until we hit the type character (d, s, v, f, etc.)
            let mut format_char = 'd';
            loop {
                match chars.peek() {
                    Some(&ch) if ch.is_ascii_digit() || ch == '0' || ch == '-' || ch == '.' => {
                        chars.next(); // skip modifier
                    }
                    Some(&ch) => {
                        format_char = ch;
                        chars.next(); // consume type char
                        break;
                    }
                    None => {
                        break;
                    }
                }
            }
            match format_char {
                'd' | 's' | 'v' | 'f' => {
                    if let Some(arg) = args_iter.next() {
                        result.push_str(&format!("{}", arg));
                    }
                }
                _ => {
                    // Unknown format specifier — output literal %<char>
                    result.push('%');
                    result.push(format_char);
                }
            }
        } else {
            result.push(c);
        }
    }

    GoString::from(result)
}

/// Go's `fmt.Print` — formatted string output to stdout.
pub fn fmt_print(args: &[String]) {
    let result = args.join(" ");
    print!("{}", result);
}

/// Go's `fmt.Println` — formatted string output to stdout with newline.
pub fn fmt_println(args: &[String]) {
    let result = args.join(" ");
    println!("{}", result);
}

/// Go's `fmt.Printf` — formatted string output to stdout (no trailing newline).
pub fn fmt_printf(format: &str, args: &[String]) {
    let result = fmt_sprintf(format, args.iter().cloned());
    print!("{}", result);
}

/// Go's `fmt.Print` with raw vec args (no format string).
pub fn fmt_print_vec(args: &[String]) {
    let result = args.join(" ");
    println!("{}", result);
}

/// Go's `fmt.Println` with raw vec args (no format string).
pub fn fmt_println_vec(args: &[String]) {
    let result = args.join(" ");
    println!("{}", result);
}