//! Optimised wire format primitives.
//!
//! Types under `#[revisioned(revision(N, encoding = "optimised", ...))]` use a
//! compact tagged-value envelope on the wire. This module provides the runtime
//! pieces the derive macro reaches into:
//!
//! - [`Tag`] / [`SizeClass`] for the 1-byte tag prefix on every optimised ADT value.
//! - [`envelope`] for inline/fixed/varlen value encoding and decoding.
//! - [`validation`] for eager prologue checks on indexed compounds.
//! - [`indexed`] for the random-access walkers ([`IndexedStructWalker`],
//!   [`IndexedMapWalker`], [`IndexedSeqWalker`]).
//!
//! User code reaches the walker types and the `Tag` / `SizeClass` pair; the rest
//! is `#[doc(hidden)]` plumbing reached only by the macro's expansion.

pub mod envelope;
pub mod indexed;
pub mod size_table;
pub mod tag;
pub mod validation;

pub use indexed::{IndexedMapWalker, IndexedSeqWalker, IndexedStructWalker, OFFSET_TABLE_MIN_LEN};
pub use tag::{MAX_VARIANTS, SizeClass, Tag};
