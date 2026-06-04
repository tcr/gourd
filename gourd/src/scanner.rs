//! Scanner module — delegates to `gourd-scanner` shared crate.

pub use gourd_scanner::{
    scan_path, scan_verify, find_go_blocks_from_source,
    GoBlock, VerifyBlock, ScanConfig,
};
