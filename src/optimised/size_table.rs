//! Per-enum static size tables for optimised fixed-class variants.
//!
//! The macro generates a `static SIZE_TABLE: [u8; N]` per enum that opts into
//! `encoding = "optimised"`, indexed by variant id. The lookup is then a single
//! array index. This module exposes the helper traits the macro reaches.

/// Lookup variant size by id. Implemented by macro-generated tables.
///
/// Returns the declared fixed-class payload size in bytes, or `None` if the
/// variant uses an inline or varlen size class (callers handle those cases
/// separately and never invoke this for them in correct codegen).
#[doc(hidden)]
pub trait OptimisedVariantSize {
	fn size_for_variant(variant_id: u8) -> Option<u8>;
}
