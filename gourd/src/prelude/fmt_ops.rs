//! Go's `fmt` package helpers.
//!
//! Provides `Sprintf`, `Print`, `Println`, and `Printf` format functions.

/// Go's `fmt.Sprintf` — formatted string output.
///
/// Supports simple format specifiers: `%d` (int), `%s` (string),
/// `%v` (value), `%f` (float).
pub fn fmt_sprintf(format: String, args: &[String]) -> String {
    let mut result = String::new();
    let mut args_iter = args.iter();

    let mut chars = format.chars().peekable();
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

    result
}

/// Go's `fmt.Print` — formatted string output to stdout.
pub fn fmt_print(format: String, args: &[String]) {
    let result = fmt_sprintf(format, args);
    println!("{}", result);
}

/// Go's `fmt.Println` — formatted string output to stdout with newline.
pub fn fmt_println(format: String, args: &[String]) {
    let result = fmt_sprintf(format, args);
    println!("{}", result);
}

/// Go's `fmt.Printf` — formatted string output to stdout (no trailing newline).
pub fn fmt_printf(format: String, args: &[String]) {
    let result = fmt_sprintf(format, args);
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
