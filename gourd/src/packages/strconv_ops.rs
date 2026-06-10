//! Go's `strconv` package helpers.
//!
//! Provides conversion functions matching Go's strconv stdlib.

/// Parses an integer string with given bit size (Go `strconv.Atoi`).
pub fn parse_int(s: String, bit_size: i32) -> Result<i64, Box<dyn std::error::Error>> {
    let i = s.parse::<i64>()?;
    match bit_size {
        8 => if i < -128 || i > 127 { return Err("value out of range for int8".into()); } else { Ok(i) },
        16 => if i < -32768 || i > 32767 { return Err("value out of range for int16".into()); } else { Ok(i) },
        32 => if i < -2147483648 || i > 2147483647 { return Err("value out of range for int32".into()); } else { Ok(i) },
        _ => Ok(i),
    }
}

/// Parses a floating-point string (Go `strconv.ParseFloat`).
pub fn parse_float(s: String, bit_size: i32) -> Result<f64, Box<dyn std::error::Error>> {
    let f = s.parse::<f64>()?;
    match bit_size {
        32 => if f.is_nan() || (f.abs() > f32::MAX as f64) { return Err("value out of range for float32".into()); } else { Ok(f) },
        _ => Ok(f),
    }
}

/// Parses a boolean string (Go `strconv.ParseBool`).
pub fn parse_bool(s: String) -> Result<bool, Box<dyn std::error::Error>> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "t" | "T" => Ok(true),
        "false" | "0" | "f" | "F" => Ok(false),
        _ => Err("invalid syntax".into()),
    }
}

/// Converts an integer to string (Go `strconv.Itoa`).
pub fn itoa(i: i64) -> String {
    i.to_string()
}

/// Converts a float to string (Go `strconv.FormatFloat`).
pub fn format_float(f: f64, fmt: String, prec: i32, _bit_size: i32) -> String {
    match fmt.as_str() {
        "b" => format_float_binary(f, prec),
        "e" => format!("{}", f),
        "f" => format!("{:.prec$}", f, prec = prec as usize),
        "g" => format!("{}", f),
        _ => format!("{}", f),
    }
}

/// Converts a float to string with binary format (Go `strconv.FormatFloat 'b'`).
pub fn format_float_binary(f: f64, prec: i32) -> String {
    format!("{:.prec$}", f, prec = prec as usize)
}

/// Converts a float to string (Go `strconv.FormatInt`).
pub fn format_int(i: i64, base: i32) -> String {
    match base {
        2 => format!("{:b}", i),
        8 => format!("{:o}", i),
        10 => i.to_string(),
        16 => format!("{:x}", i),
        _ => i.to_string(),
    }
}

/// Converts a byte to string (Go `strconv.FormatByte`).
pub fn format_byte(b: u8) -> String {
    b.to_string()
}

/// Converts a rune to string (Go `strconv.FormatRune`).
pub fn format_rune(r: char) -> String {
    r.to_string()
}

/// Converts a boolean to string (Go `strconv.FormatBool`).
pub fn format_bool(b: bool) -> String {
    b.to_string()
}

/// Appends an integer as string (Go `strconv.AppendInt`).
pub fn append_int(mut s: String, i: i64, base: i32) -> String {
    s.push_str(&format_int(i, base));
    s
}

/// Appends a float as string (Go `strconv.AppendFloat`).
pub fn append_float(mut s: String, f: f64, fmt: String, prec: i32, _bit_size: i32) -> String {
    s.push_str(&format_float(f, fmt, prec, _bit_size));
    s
}

/// Appends a bool as string (Go `strconv.AppendBool`).
pub fn append_bool(mut s: String, b: bool) -> String {
    s.push_str(&format_bool(b));
    s
}
