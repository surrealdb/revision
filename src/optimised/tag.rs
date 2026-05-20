//! Tag byte for the optimised wire format.
//!
//! Every ADT value under `optimised` begins with a single tag byte:
//!
//! ```text
//! u8 tag:
//!   bits 0..=4  variant id   (5-bit; max 32 variants per enum)
//!   bits 5..=6  size class:
//!                 0b00 inline   (no payload — None, Null, true, false, EmptyArray, ...)
//!                 0b01 fixed    (static-sized payload per variant)
//!                 0b10 varlen   (u32_le byte_length || payload)
//!                 0b11 reserved (decode error: InvalidOptimisedTag)
//!   bit  7      reserved (extended-tag escape; must be 0)
//! ```

use std::io::{Read, Write};

use crate::Error;

const VARIANT_ID_MASK: u8 = 0b0001_1111;
const SIZE_CLASS_MASK: u8 = 0b0110_0000;
const SIZE_CLASS_SHIFT: u8 = 5;
const EXTENDED_BIT: u8 = 0b1000_0000;

const SIZE_CLASS_INLINE: u8 = 0b00;
const SIZE_CLASS_FIXED: u8 = 0b01;
const SIZE_CLASS_VARLEN: u8 = 0b10;

/// Maximum number of variants an enum can declare under `optimised`.
pub const MAX_VARIANTS: usize = 32;

/// Size class of an optimised value: how the payload (if any) is encoded after the tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SizeClass {
	/// No payload — the tag byte is the entire encoding.
	Inline,
	/// Payload size known statically from the per-enum size table.
	Fixed,
	/// Payload preceded by a `u32_le` byte length.
	Varlen,
}

impl SizeClass {
	#[inline]
	const fn to_bits(self) -> u8 {
		match self {
			SizeClass::Inline => SIZE_CLASS_INLINE,
			SizeClass::Fixed => SIZE_CLASS_FIXED,
			SizeClass::Varlen => SIZE_CLASS_VARLEN,
		}
	}

	#[inline]
	const fn from_bits(bits: u8) -> Result<Self, ()> {
		match bits {
			SIZE_CLASS_INLINE => Ok(SizeClass::Inline),
			SIZE_CLASS_FIXED => Ok(SizeClass::Fixed),
			SIZE_CLASS_VARLEN => Ok(SizeClass::Varlen),
			_ => Err(()),
		}
	}
}

/// Optimised value tag byte. See module docs for the bit layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tag(pub u8);

impl Tag {
	/// Build a tag from a variant id and size class.
	///
	/// Panics in debug builds if `variant_id >= MAX_VARIANTS`. Release builds
	/// silently truncate to 5 bits — the caller must have validated this at
	/// macro expansion time.
	#[inline]
	pub const fn new(variant_id: u8, sc: SizeClass) -> Self {
		debug_assert!(variant_id < MAX_VARIANTS as u8);
		Tag((variant_id & VARIANT_ID_MASK) | (sc.to_bits() << SIZE_CLASS_SHIFT))
	}

	/// Variant id (bits 0..=4).
	#[inline]
	pub const fn variant_id(self) -> u8 {
		self.0 & VARIANT_ID_MASK
	}

	/// Size class (bits 5..=6). Returns an error if the reserved `0b11` is observed.
	#[inline]
	pub const fn size_class(self) -> Result<SizeClass, Error> {
		match SizeClass::from_bits((self.0 & SIZE_CLASS_MASK) >> SIZE_CLASS_SHIFT) {
			Ok(sc) => Ok(sc),
			Err(()) => Err(Error::InvalidOptimisedTag(self.0)),
		}
	}

	/// Whether the extended-tag bit is set (reserved for future encodings).
	#[inline]
	pub const fn is_extended(self) -> bool {
		self.0 & EXTENDED_BIT != 0
	}
}

#[doc(hidden)]
#[inline]
pub fn read_tag<R: Read>(r: &mut R) -> Result<Tag, Error> {
	let mut buf = [0u8; 1];
	r.read_exact(&mut buf).map_err(Error::Io)?;
	Ok(Tag(buf[0]))
}

#[doc(hidden)]
#[inline]
pub fn write_tag<W: Write>(w: &mut W, t: Tag) -> Result<(), Error> {
	w.write_all(&[t.0]).map_err(Error::Io)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn tag_round_trip_inline() {
		let t = Tag::new(7, SizeClass::Inline);
		assert_eq!(t.variant_id(), 7);
		assert_eq!(t.size_class().unwrap(), SizeClass::Inline);
		assert!(!t.is_extended());
	}

	#[test]
	fn tag_round_trip_fixed_max_variant() {
		let t = Tag::new(31, SizeClass::Fixed);
		assert_eq!(t.variant_id(), 31);
		assert_eq!(t.size_class().unwrap(), SizeClass::Fixed);
	}

	#[test]
	fn tag_round_trip_varlen() {
		let t = Tag::new(0, SizeClass::Varlen);
		assert_eq!(t.variant_id(), 0);
		assert_eq!(t.size_class().unwrap(), SizeClass::Varlen);
	}

	#[test]
	fn tag_reserved_size_class_errors() {
		// 0b11 in size-class bits — never produced by `new`, but a corrupt wire could.
		let raw = 0b0110_0000;
		let t = Tag(raw);
		assert!(matches!(t.size_class(), Err(Error::InvalidOptimisedTag(b)) if b == raw));
	}

	#[test]
	fn tag_extended_bit_detected() {
		let t = Tag(0b1000_0000 | Tag::new(1, SizeClass::Inline).0);
		assert!(t.is_extended());
	}

	#[test]
	fn read_write_tag_round_trips() {
		let tag = Tag::new(13, SizeClass::Varlen);
		let mut buf = Vec::new();
		write_tag(&mut buf, tag).unwrap();
		assert_eq!(buf.len(), 1);
		let mut cursor: &[u8] = &buf;
		let read = read_tag(&mut cursor).unwrap();
		assert_eq!(read, tag);
	}

	#[test]
	#[should_panic]
	fn tag_new_panics_on_overflow_in_debug() {
		let _ = Tag::new(32, SizeClass::Inline);
	}
}
