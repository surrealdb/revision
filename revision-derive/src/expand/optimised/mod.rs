//! Code generation for `encoding = "optimised"` revisions.
//!
//! Each `revision(N, encoding = "optimised", ...)` entry in a type's history
//! routes through this module instead of the legacy `expand/ser.rs`,
//! `expand/de.rs`, `expand/skip.rs` codegen. Walker codegen still lives in
//! `expand/walk.rs`; this module supplies the new arms it dispatches into.

mod struct_codegen;

pub use struct_codegen::{emit_struct_deserialize, emit_struct_serialize, emit_struct_skip};
