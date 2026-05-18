//! Hand-crafted byte-level tests for the optimised wire format primitives.
//!
//! These tests exercise the runtime helpers in `revision::optimised::*` without
//! going through the derive macro. End-to-end macro tests live in
//! `tests/macro_optimised.rs` (added in Step 10).

use revision::optimised::envelope::{
	encode_fixed, encode_inline, encode_varlen, read_optimised_tag, read_varlen_slice, skip_varlen,
	skip_varlen_borrowed,
};
use revision::optimised::tag::{MAX_VARIANTS, SizeClass, Tag, read_tag, write_tag};
use revision::slice_reader::SliceReader;

#[test]
fn inline_value_is_one_byte_total() {
	let mut buf = Vec::new();
	encode_inline(&mut buf, 0).unwrap();
	assert_eq!(buf, [Tag::new(0, SizeClass::Inline).0]);
}

#[test]
fn inline_at_max_variant_id() {
	let mut buf = Vec::new();
	encode_inline(&mut buf, (MAX_VARIANTS - 1) as u8).unwrap();
	assert_eq!(buf.len(), 1);
	let mut r: &[u8] = &buf;
	let (tag, sc) = read_optimised_tag(&mut r).unwrap();
	assert_eq!(tag.variant_id(), (MAX_VARIANTS - 1) as u8);
	assert_eq!(sc, SizeClass::Inline);
}

#[test]
fn fixed_value_emits_tag_plus_static_payload() {
	let mut buf = Vec::new();
	encode_fixed(&mut buf, 5, |w| {
		std::io::Write::write_all(w, &123456789i64.to_le_bytes()).map_err(revision::Error::Io)
	})
	.unwrap();
	assert_eq!(buf.len(), 9);
	let mut r: &[u8] = &buf;
	let (tag, sc) = read_optimised_tag(&mut r).unwrap();
	assert_eq!(tag.variant_id(), 5);
	assert_eq!(sc, SizeClass::Fixed);
	let mut payload = [0u8; 8];
	std::io::Read::read_exact(&mut r, &mut payload).unwrap();
	assert_eq!(i64::from_le_bytes(payload), 123456789);
}

#[test]
fn varlen_value_emits_tag_length_payload() {
	let body = b"hello, optimised wire";
	let mut buf = Vec::new();
	encode_varlen(&mut buf, 9, |scratch| {
		scratch.extend_from_slice(body);
		Ok(())
	})
	.unwrap();
	// 1 tag + 4 u32_le length + body
	assert_eq!(buf.len(), 1 + 4 + body.len());
	let mut r = SliceReader::new(&buf);
	let (tag, sc) = read_optimised_tag(&mut r).unwrap();
	assert_eq!(tag.variant_id(), 9);
	assert_eq!(sc, SizeClass::Varlen);
	let payload = read_varlen_slice(&mut r).unwrap();
	assert_eq!(payload, body);
}

#[test]
fn varlen_skip_paths_advance_full_length() {
	let mut buf = Vec::new();
	encode_varlen(&mut buf, 0, |s| {
		s.extend_from_slice(&[0xCD; 256]);
		Ok(())
	})
	.unwrap();

	// Skip via `Read` (streaming path).
	let mut r: &[u8] = &buf;
	let _ = read_optimised_tag(&mut r).unwrap();
	skip_varlen(&mut r).unwrap();
	assert!(r.is_empty());

	// Skip via `BorrowedReader` (slice path).
	let mut r2 = SliceReader::new(&buf);
	let _ = read_optimised_tag(&mut r2).unwrap();
	skip_varlen_borrowed(&mut r2).unwrap();
	assert!(r2.remaining().is_empty());
}

#[test]
fn read_tag_then_write_tag_round_trips() {
	let tag = Tag::new(17, SizeClass::Fixed);
	let mut buf = Vec::new();
	write_tag(&mut buf, tag).unwrap();
	let mut r: &[u8] = &buf;
	let read = read_tag(&mut r).unwrap();
	assert_eq!(read, tag);
}

#[test]
fn nested_varlen_round_trips() {
	let mut buf = Vec::new();
	encode_varlen(&mut buf, 1, |outer| {
		encode_varlen(outer, 2, |inner| {
			inner.extend_from_slice(b"nested payload");
			Ok(())
		})
	})
	.unwrap();
	let mut r = SliceReader::new(&buf);
	let (outer_tag, _) = read_optimised_tag(&mut r).unwrap();
	assert_eq!(outer_tag.variant_id(), 1);
	let outer_payload = read_varlen_slice(&mut r).unwrap();
	let mut inner_r: &[u8] = outer_payload;
	let (inner_tag, _) = read_optimised_tag(&mut inner_r).unwrap();
	assert_eq!(inner_tag.variant_id(), 2);
	let inner_payload = revision::optimised::envelope::read_varlen_len(&mut inner_r).unwrap();
	assert_eq!(inner_payload as usize, b"nested payload".len());
}
