//! Go's `log` package helpers.
//!
//! Provides logging utilities matching Go's log stdlib, using Rust's `log` crate.

/// Log a message at INFO level (Go `log.Print`).
pub fn log_print(message: String) {
    log::info!("{}", message);
}

/// Log a formatted message at INFO level (Go `log.Printf`).
pub fn log_printf(format: String, args: Vec<String>) -> String {
    let mut result = format.clone();
    for arg in args {
        result = result.replacen("{}", &arg, 1);
    }
    log::info!("{}", result);
    result
}

/// Log a message with newline at INFO level (Go `log.Println`).
pub fn log_println(message: String) {
    log::info!("{}!", message);
}

/// Log at ERROR level (Go `log.Fatal` equivalent).
pub fn log_fatal(message: String) {
    log::error!("{}", message);
}

/// Log a formatted error message (Go `log.Fatalf` equivalent).
pub fn logf_fatal(format: String, args: Vec<String>) -> String {
    let mut result = format.clone();
    for arg in args {
        result = result.replacen("{}", &arg, 1);
    }
    log::error!("{}", result);
    result
}

/// Log an error with newline (Go `log.Fatalln` equivalent).
pub fn logln_fatal(message: String) {
    log::error!("{}!", message);
}

/// Set the logger prefix (Go `log.SetPrefix`).
pub fn set_prefix(prefix: String) -> String {
    // Store prefix in a static variable if needed, or just log it
    let _ = format!("PREFIX: {}", prefix);
    prefix
}

/// Get the current logger prefix (Go `log.Prefix`).
pub fn prefix() -> String {
    "".to_string()
}

/// Output the log flag (Go `log.Flags`).
pub fn flags() -> i32 {
    // Ldate | Ltime | Lmicroseconds = 0x00123
    0x00123
}

/// Set the log flags (Go `log.SetFlags`).
pub fn set_flags(flag: i32) -> String {
    let mut flags = Vec::new();
    if flag & 0x001 != 0 { flags.push("Ldate"); }
    if flag & 0x002 != 0 { flags.push("Ltime"); }
    if flag & 0x004 != 0 { flags.push("Lmicroseconds"); }
    if flag & 0x008 != 0 { flags.push("Llongfile"); }
    if flag & 0x010 != 0 { flags.push("Lshortfile"); }
    if flag & 0x020 != 0 { flags.push("LUTC"); }
    if flag & 0x040 != 0 { flags.push("LstdFlags"); }
    format!("{:?}", flags)
}

/// Output the caller information (Go `log.Output`).
pub fn output(_calldepth: i32, s: String) -> String {
    // In a real implementation, this would use the log crate's Location feature
    s
}

/// Set the output destination (Go `log.SetOutput`).
pub fn set_output(target: String) -> String {
    // In a real implementation, this would set up the log writer
    format!("Writer: {}", target)
}

/// Initialize the logger (Go `log.Init`).
pub fn init() {
    // In a real implementation, this would set up the default logger
}

/// Re-enable previous panic behavior (Go `log.SetPanic`).
pub fn set_panic(flag: bool) -> String {
    if flag { "Panic enabled".to_string() } else { "Panic disabled".to_string() }
}
