//! Go's `io` package helpers.
//!
//! Provides copy and read-all operations.

/// Go's `io.Copy(dst, src)` — copies min(dst.len(), src.len()) bytes from src to dst, returns count.
/// Like Go's io.Copy but operates on Vec<u8> instead of io.Reader/io.Writer.
pub fn io_copy(dst: &mut Vec<u8>, src: &[u8]) -> i64 {
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
    n as i64
}

/// Go's `io.ReadAll(reader)` — reads all bytes from a reader.
pub fn io_read_all(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}
