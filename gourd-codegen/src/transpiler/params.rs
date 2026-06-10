//! Go parameter, output, function, struct, and interface parsing.
//! Re-exports from HIR — HIR defines both types and Parse impls.

pub(crate) use super::hir::ast::{GoFn, GoFnInputs, GoFnOutput, GoInterface, GoInterfaceMethod, GoParam, GoStruct, GoStructField};
