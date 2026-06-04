//! Go's `io` package helpers.
//!
//! Provides copy and read-all operations.

/// Go's `io.Copy(dst, src)` — copies from src to dst, returns bytes copied.
pub fn io_copy(dst: &mut Vec<u8>, src: &[u8]) -> i64 {
    let n = src.len().min(dst.capacity() - dst.len());
    dst.extend_from_slice(&src[..n]);
    n as i64
}

/// Go's `io.ReadAll(reader)` — reads all bytes from a reader.
pub fn io_read_all(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}
