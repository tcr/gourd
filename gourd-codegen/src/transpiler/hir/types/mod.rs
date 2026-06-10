//! HIR Type module.
//!
//! Provides type definitions, parsing, and mapping functions for Go types.

pub(crate) mod primitives;
pub(crate) mod mapping;
pub(crate) mod compound;

// Re-export types for backward compatibility
pub(crate) use primitives::{ HirType, HirTypeKind, HirInterfaceMethod, HirReceiverFn };
pub(crate) use mapping::{ go_type_to_hir, parse_go_type, parse_go_struct, parse_go_interface };
pub(crate) use compound::{ HirSelect, HirSelectCase, HirSwitch, HirSwitchCase, map_go_type_str, map_go_types, HirFunction, HirStruct, parse_go_receiver_fn };