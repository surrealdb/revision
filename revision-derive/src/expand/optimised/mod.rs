//! Code generation for `optimised` revisions.
//!
//! Each `revision(N, optimised, ...)` entry in a type's history
//! routes through this module instead of the legacy `expand/ser.rs`,
//! `expand/de.rs`, `expand/skip.rs` codegen. Walker codegen still lives in
//! `expand/walk.rs`; this module supplies the new arms it dispatches into.

mod enum_codegen;
mod struct_codegen;

pub use enum_codegen::{emit_enum_deserialize, emit_enum_serialize, emit_enum_skip};
pub use struct_codegen::{emit_struct_deserialize, emit_struct_serialize, emit_struct_skip};
