//! Re-export Go AST types from HIR — legacy callers use these.

pub(crate) use super::hir::ast::{GoBlock, GoFn, GoFnInputs, GoFnOutput, GoIf, GoInterface, GoInterfaceMethod, GoParam, GoSelect, GoSelectCase, GoStmt, GoStruct, GoStructField, Switch, SwitchCase};
