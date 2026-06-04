//! Scanner module — re-exports from `gourd-codegen`.

pub use gourd_codegen::scanner::{
    scan_path, scan_verify, find_go_blocks_from_source,
    GoBlock, VerifyBlock, ScanConfig,
};
