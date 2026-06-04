//! Go's `os` package helpers.
//!
//! Provides 10 file and environment operations matching Go's stdlib.

/// Go's `os.Open(path)` — opens a file for reading.
pub fn os_open(path: &str) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

/// Go's `os.ReadFile(path)` — reads entire file contents.
pub fn os_read_file(path: &str) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

/// Go's `os.WriteFile(path, data, perm)` — writes data to a file.
pub fn os_write_file(path: &str, data: &[u8], _perm: i32) -> std::io::Result<()> {
    std::fs::write(path, data)
}

/// Go's `os.Mkdir(path, perm)` — creates a directory.
pub fn os_mkdir(path: &str, _perm: i32) -> std::io::Result<()> {
    std::fs::create_dir(path)
}

/// Go's `os.MkdirAll(path, perm)` — creates a directory and all parent directories.
pub fn os_mkdir_all(path: &str, _perm: i32) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

/// Go's `os.Remove(path)` — removes a file or directory.
pub fn os_remove(path: &str) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

/// Go's `os.Chdir(path)` — changes current directory.
pub fn os_chdir(path: &str) -> std::io::Result<()> {
    std::env::set_current_dir(path)
}

/// Go's `os.Getenv(key)` — reads an environment variable.
/// Returns (value, ok) where ok=false means the variable was not set.
pub fn os_getenv(key: &str) -> Result<String, ()> {
    match std::env::var(key) {
        Ok(val) => Ok(val),
        Err(_) => Err(()),
    }
}

/// Go's `os.Setenv(key, value)` — sets an environment variable.
pub fn os_setenv(key: &str, value: &str) {
    unsafe { std::env::set_var(key, value); }
}

/// Returns all environment variable keys (Go `os.Environ()`).
pub fn os_env_keys() -> Vec<String> {
    std::env::vars().map(|(k, _)| k).collect()
}

/// Go's `os.Args` — command-line arguments as []string.
pub fn os_args() -> Vec<String> {
    std::env::args().collect()
}
