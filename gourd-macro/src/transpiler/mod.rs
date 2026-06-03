//! Transpiler module.
//!
//! Re-exports from `gourd-codegen` — the proc-macro crate delegates
//! all transpilation logic to the core crate to work around the
//! proc-macro crate limitation (can't re-export non-proc-macro items).

pub use gourd_codegen::transpiler::{
    free_fn::{go_to_rust_fn, go_to_rust_struct, go_to_rust_switch},
    funcs::go_to_rust_receiver_fn,
    transpile_go,
};
