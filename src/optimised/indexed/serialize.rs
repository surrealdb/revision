//! Encode side of indexed compounds.
//!
//! These helpers produce wire bytes that [`IndexedMapWalker`] and
//! [`IndexedSeqWalker`] expect on the read side. Each takes a target writer
//! and the data structure to serialise; the K, V, T types serialise their
//! components via `SerializeRevisioned`.
//!
//! [`IndexedMapWalker`]: crate::optimised::IndexedMapWalker
//! [`IndexedSeqWalker`]: crate::optimised::IndexedSeqWalker

use std::collections::BTreeMap;
use std::io::{Read, Write};

use crate::Error;
use crate::SkipRevisioned;
use crate::optimised::indexed::seq_walk::FLAG_INDEXED;
use crate::{DeserializeRevisioned, SerializeRevisioned};

// -----------------------------------------------------------------------------
// Trait surface
// -----------------------------------------------------------------------------
//
// The macro emits `<FieldType as IndexedMapEncoded>::...` for fields tagged
// `#[revision(indexed_map)]`, and likewise for `IndexedSeqEncoded`. The free
// functions below (`serialize_indexed_map`, ...) remain the public surface for
// hand-written impls; the trait blanket-delegates to them.

/// Map-shaped types that opt into the indexed wire format under optimised
/// revisions. Implemented for [`BTreeMap`] out of the box.
#[doc(hidden)]
pub trait IndexedMapEncoded: Sized {
	fn serialize_indexed_map<W: Write>(&self, w: &mut W) -> Result<(), Error>;
	fn deserialize_indexed_map<R: Read>(r: &mut R) -> Result<Self, Error>;
	fn skip_indexed_map<R: Read>(r: &mut R) -> Result<(), Error>;
}

impl<K, V> IndexedMapEncoded for BTreeMap<K, V>
where
	K: SerializeRevisioned + DeserializeRevisioned + SkipRevisioned + Ord,
	V: SerializeRevisioned + DeserializeRevisioned + SkipRevisioned,
{
	fn serialize_indexed_map<W: Write>(&self, w: &mut W) -> Result<(), Error> {
		serialize_indexed_map(self, w)
	}
	fn deserialize_indexed_map<R: Read>(r: &mut R) -> Result<Self, Error> {
		deserialize_indexed_map(r)
	}
	fn skip_indexed_map<R: Read>(r: &mut R) -> Result<(), Error> {
		skip_indexed_map::<K, V, R>(r)
	}
}

/// Sequence-shaped types under optimised. Implemented for [`Vec`].
#[doc(hidden)]
pub trait IndexedSeqEncoded: Sized {
	fn serialize_indexed_seq<W: Write>(&self, w: &mut W) -> Result<(), Error>;
	fn deserialize_indexed_seq<R: Read>(r: &mut R) -> Result<Self, Error>;
	fn skip_indexed_seq<R: Read>(r: &mut R) -> Result<(), Error>;
}

impl<T> IndexedSeqEncoded for Vec<T>
where
	T: SerializeRevisioned + DeserializeRevisioned + SkipRevisioned,
{
	fn serialize_indexed_seq<W: Write>(&self, w: &mut W) -> Result<(), Error> {
		serialize_indexed_seq(self, w)
	}
	fn deserialize_indexed_seq<R: Read>(r: &mut R) -> Result<Self, Error> {
		deserialize_indexed_seq(r)
	}
	fn skip_indexed_seq<R: Read>(r: &mut R) -> Result<(), Error> {
		skip_indexed_seq::<T, R>(r)
	}
}

/// Wire layout produced:
///
/// ```text
/// u8 flags                                  // bit 0: indexed
/// varint len                                // entry count
/// [(u32_le key_off, u32_le val_off); len]   // offset table
/// u32_le keys_region_len
/// u32_le vals_region_len
/// dense keys (sorted, ascending)
/// dense values (same order as keys)
/// ```
///
/// Each key and value is serialised via `SerializeRevisioned`. Keys are
/// emitted in ascending byte-compare order — `BTreeMap` already iterates in
/// sorted key order, so no extra sort cost on the encode path.
#[doc(hidden)]
pub fn serialize_indexed_map<K, V, W: Write>(
	map: &BTreeMap<K, V>,
	writer: &mut W,
) -> Result<(), Error>
where
	K: SerializeRevisioned,
	V: SerializeRevisioned,
{
	let len = map.len();
	// Pre-serialise each entry so we know the offsets and region sizes.
	// IMPORTANT: `BTreeMap` iterates in K-order, but the indexed wire format
	// requires the keys region to be ascending under *byte* compare (what the
	// IndexedMapWalker uses for binary search). For variable-length key types
	// like `String` whose `SerializeRevisioned` emits `varint(len) || bytes`,
	// K-order and byte-order diverge whenever the varint length differs. We
	// therefore sort the pre-serialised entries by key bytes before writing.
	let mut pairs: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(len);
	for (k, v) in map.iter() {
		let mut kb = Vec::new();
		k.serialize_revisioned(&mut kb)?;
		let mut vb = Vec::new();
		v.serialize_revisioned(&mut vb)?;
		pairs.push((kb, vb));
	}
	pairs.sort_by(|a, b| a.0.cmp(&b.0));
	let (keys, vals): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();

	// Header: flags = indexed, varint length.
	writer.write_all(&[FLAG_INDEXED]).map_err(Error::Io)?;
	write_varint(writer, len)?;

	// Compute the two offset tables in parallel.
	let mut k_off = 0u32;
	let mut v_off = 0u32;
	let mut k_offsets = Vec::with_capacity(len);
	let mut v_offsets = Vec::with_capacity(len);
	for (kb, vb) in keys.iter().zip(vals.iter()) {
		k_offsets.push(k_off);
		v_offsets.push(v_off);
		k_off = k_off
			.checked_add(kb.len() as u32)
			.ok_or_else(|| Error::Serialize("indexed map key region exceeds u32::MAX".into()))?;
		v_off = v_off
			.checked_add(vb.len() as u32)
			.ok_or_else(|| Error::Serialize("indexed map value region exceeds u32::MAX".into()))?;
	}

	// Interleave (k_off, v_off) pairs to match the walker layout.
	for i in 0..len {
		writer.write_all(&k_offsets[i].to_le_bytes()).map_err(Error::Io)?;
		writer.write_all(&v_offsets[i].to_le_bytes()).map_err(Error::Io)?;
	}
	// Region lengths.
	writer.write_all(&k_off.to_le_bytes()).map_err(Error::Io)?;
	writer.write_all(&v_off.to_le_bytes()).map_err(Error::Io)?;
	// Dense keys.
	for kb in &keys {
		writer.write_all(kb).map_err(Error::Io)?;
	}
	// Dense values.
	for vb in &vals {
		writer.write_all(vb).map_err(Error::Io)?;
	}
	Ok(())
}

/// Wire layout produced:
///
/// ```text
/// u8 flags                       // bit 0: indexed
/// varint len                     // element count
/// [u32_le elem_off; len]         // offset table
/// elements concatenated
/// ```
///
/// Each element is serialised via `SerializeRevisioned`.
#[doc(hidden)]
pub fn serialize_indexed_seq<T, W: Write>(items: &[T], writer: &mut W) -> Result<(), Error>
where
	T: SerializeRevisioned,
{
	let len = items.len();
	let mut bodies: Vec<Vec<u8>> = Vec::with_capacity(len);
	for item in items {
		let mut b = Vec::new();
		item.serialize_revisioned(&mut b)?;
		bodies.push(b);
	}

	writer.write_all(&[FLAG_INDEXED]).map_err(Error::Io)?;
	write_varint(writer, len)?;

	let mut off = 0u32;
	for b in &bodies {
		writer.write_all(&off.to_le_bytes()).map_err(Error::Io)?;
		off = off
			.checked_add(b.len() as u32)
			.ok_or_else(|| Error::Serialize("indexed seq exceeds u32::MAX".into()))?;
	}
	for b in &bodies {
		writer.write_all(b).map_err(Error::Io)?;
	}
	Ok(())
}

/// Decode an indexed map written by [`serialize_indexed_map`].
///
/// The offset tables and region lengths are *random-access metadata* used by
/// [`IndexedMapWalker`]; the sequential deserializer skips past them and
/// reads keys and values directly via `DeserializeRevisioned`, which knows
/// its own size per item. This keeps the function readable from any `Read`
/// (no need to bound the body up front) and matches what a sibling field's
/// deserializer expects of the input cursor.
///
/// [`IndexedMapWalker`]: crate::optimised::IndexedMapWalker
#[doc(hidden)]
pub fn deserialize_indexed_map<K, V, R: Read>(reader: &mut R) -> Result<BTreeMap<K, V>, Error>
where
	K: DeserializeRevisioned + Ord,
	V: DeserializeRevisioned,
{
	let mut flag_buf = [0u8; 1];
	reader.read_exact(&mut flag_buf).map_err(Error::Io)?;
	let flags = flag_buf[0];
	let len = read_varint(reader)?;
	if (flags & FLAG_INDEXED) == 0 {
		// Legacy `(K, V)*` fallback inside the indexed-flag header.
		let mut out = BTreeMap::new();
		for _ in 0..len {
			let k = K::deserialize_revisioned(reader)?;
			let v = V::deserialize_revisioned(reader)?;
			out.insert(k, v);
		}
		return Ok(out);
	}

	// Skip the offset table (len * 8) and region-length pair (8 bytes); we
	// don't need them for sequential decode.
	let table_bytes = len.checked_mul(8).ok_or(Error::OptimisedSubReaderOverrun)?;
	let mut discard = vec![0u8; table_bytes + 8];
	reader.read_exact(&mut discard).map_err(Error::Io)?;

	// Dense keys (sorted ascending) come first, then dense values in matching
	// order. Each K and V know their own wire length via DeserializeRevisioned.
	let mut keys: Vec<K> = Vec::with_capacity(len);
	for _ in 0..len {
		keys.push(K::deserialize_revisioned(reader)?);
	}
	let mut values: Vec<V> = Vec::with_capacity(len);
	for _ in 0..len {
		values.push(V::deserialize_revisioned(reader)?);
	}
	let mut out = BTreeMap::new();
	for (k, v) in keys.into_iter().zip(values) {
		out.insert(k, v);
	}
	Ok(out)
}

/// Decode an indexed sequence written by [`serialize_indexed_seq`].
///
/// As with the map decoder, the offset table is metadata for the walker; the
/// sequential decoder skips it and reads elements one by one. Each element's
/// own `DeserializeRevisioned` impl bounds its read.
#[doc(hidden)]
pub fn deserialize_indexed_seq<T, R: Read>(reader: &mut R) -> Result<Vec<T>, Error>
where
	T: DeserializeRevisioned,
{
	let mut flag_buf = [0u8; 1];
	reader.read_exact(&mut flag_buf).map_err(Error::Io)?;
	let flags = flag_buf[0];
	let len = read_varint(reader)?;
	if (flags & FLAG_INDEXED) == 0 {
		// Legacy fallback: pure `(elem)*` body.
		let mut out = Vec::with_capacity(len);
		for _ in 0..len {
			out.push(T::deserialize_revisioned(reader)?);
		}
		return Ok(out);
	}

	// Skip the offset table (len * 4 bytes).
	let table_bytes = len.checked_mul(4).ok_or(Error::OptimisedSubReaderOverrun)?;
	let mut discard = vec![0u8; table_bytes];
	reader.read_exact(&mut discard).map_err(Error::Io)?;

	let mut out = Vec::with_capacity(len);
	for _ in 0..len {
		out.push(T::deserialize_revisioned(reader)?);
	}
	Ok(out)
}

/// Advance past an indexed-map encoding without materialising the keys or values.
///
/// Mirrors [`deserialize_indexed_map`] structurally: read flags, len, skip
/// the offset table + region lengths, then call `K::skip_revisioned` and
/// `V::skip_revisioned` `len` times each.
#[doc(hidden)]
pub fn skip_indexed_map<K, V, R: Read>(reader: &mut R) -> Result<(), Error>
where
	K: SkipRevisioned,
	V: SkipRevisioned,
{
	let mut flag_buf = [0u8; 1];
	reader.read_exact(&mut flag_buf).map_err(Error::Io)?;
	let flags = flag_buf[0];
	let len = read_varint(reader)?;
	if (flags & FLAG_INDEXED) == 0 {
		for _ in 0..len {
			K::skip_revisioned(reader)?;
			V::skip_revisioned(reader)?;
		}
		return Ok(());
	}
	let table_bytes = len.checked_mul(8).ok_or(Error::OptimisedSubReaderOverrun)?;
	let mut discard = vec![0u8; table_bytes + 8];
	reader.read_exact(&mut discard).map_err(Error::Io)?;
	for _ in 0..len {
		K::skip_revisioned(reader)?;
	}
	for _ in 0..len {
		V::skip_revisioned(reader)?;
	}
	Ok(())
}

/// Advance past an indexed-seq encoding.
#[doc(hidden)]
pub fn skip_indexed_seq<T, R: Read>(reader: &mut R) -> Result<(), Error>
where
	T: SkipRevisioned,
{
	let mut flag_buf = [0u8; 1];
	reader.read_exact(&mut flag_buf).map_err(Error::Io)?;
	let flags = flag_buf[0];
	let len = read_varint(reader)?;
	if (flags & FLAG_INDEXED) == 0 {
		for _ in 0..len {
			T::skip_revisioned(reader)?;
		}
		return Ok(());
	}
	let table_bytes = len.checked_mul(4).ok_or(Error::OptimisedSubReaderOverrun)?;
	let mut discard = vec![0u8; table_bytes];
	reader.read_exact(&mut discard).map_err(Error::Io)?;
	for _ in 0..len {
		T::skip_revisioned(reader)?;
	}
	Ok(())
}

#[doc(hidden)]
fn read_varint<R: Read>(r: &mut R) -> Result<usize, Error> {
	let mut tag_buf = [0u8; 1];
	r.read_exact(&mut tag_buf).map_err(Error::Io)?;
	let tag = tag_buf[0];
	match tag {
		0..=250 => Ok(tag as usize),
		251 => {
			let mut b = [0u8; 2];
			r.read_exact(&mut b).map_err(Error::Io)?;
			Ok(u16::from_le_bytes(b) as usize)
		}
		252 => {
			let mut b = [0u8; 4];
			r.read_exact(&mut b).map_err(Error::Io)?;
			Ok(u32::from_le_bytes(b) as usize)
		}
		253 => {
			let mut b = [0u8; 8];
			r.read_exact(&mut b).map_err(Error::Io)?;
			let v = u64::from_le_bytes(b);
			usize::try_from(v).map_err(|_| Error::IntegerOverflow)
		}
		_ => Err(Error::InvalidIntegerEncoding),
	}
}

#[doc(hidden)]
fn write_varint<W: Write>(w: &mut W, v: usize) -> Result<(), Error> {
	if v <= 250 {
		w.write_all(&[v as u8]).map_err(Error::Io)
	} else if v <= u16::MAX as usize {
		w.write_all(&[251]).map_err(Error::Io)?;
		w.write_all(&(v as u16).to_le_bytes()).map_err(Error::Io)
	} else if v <= u32::MAX as usize {
		w.write_all(&[252]).map_err(Error::Io)?;
		w.write_all(&(v as u32).to_le_bytes()).map_err(Error::Io)
	} else {
		w.write_all(&[253]).map_err(Error::Io)?;
		w.write_all(&(v as u64).to_le_bytes()).map_err(Error::Io)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::optimised::{IndexedMapWalker, IndexedSeqWalker};

	#[test]
	fn round_trip_indexed_map_of_strings() {
		let mut map: BTreeMap<String, u32> = BTreeMap::new();
		map.insert("alpha".into(), 1);
		map.insert("bravo".into(), 2);
		map.insert("charlie".into(), 3);

		let mut bytes = Vec::new();
		serialize_indexed_map(&map, &mut bytes).unwrap();

		let walker: IndexedMapWalker<String, u32> = IndexedMapWalker::from_payload(&bytes).unwrap();
		assert!(walker.is_indexed());
		assert_eq!(walker.len(), 3);

		// Iterate in ascending order.
		let entries: Vec<(&[u8], &[u8])> = walker.entries().unwrap().collect();
		assert_eq!(entries.len(), 3);
	}

	#[test]
	fn round_trip_indexed_seq() {
		let items: Vec<u32> = vec![10, 20, 30];
		let mut bytes = Vec::new();
		serialize_indexed_seq(&items, &mut bytes).unwrap();

		let walker: IndexedSeqWalker<u32> = IndexedSeqWalker::from_payload(&bytes).unwrap();
		assert!(walker.is_indexed());
		assert_eq!(walker.len(), 3);
	}

	#[test]
	fn serialize_then_deserialize_indexed_map_round_trips() {
		let mut original: BTreeMap<String, u32> = BTreeMap::new();
		for (i, s) in ["alpha", "bravo", "charlie", "delta"].iter().enumerate() {
			original.insert(s.to_string(), i as u32);
		}
		let mut bytes = Vec::new();
		serialize_indexed_map(&original, &mut bytes).unwrap();
		let mut r: &[u8] = &bytes;
		let decoded: BTreeMap<String, u32> = deserialize_indexed_map(&mut r).unwrap();
		assert_eq!(decoded, original);
		assert!(r.is_empty(), "deserialize should consume the whole input");
	}

	#[test]
	fn serialize_then_deserialize_indexed_seq_round_trips() {
		let original: Vec<u32> = vec![10, 20, 30, 40, 50];
		let mut bytes = Vec::new();
		serialize_indexed_seq(&original, &mut bytes).unwrap();
		let mut r: &[u8] = &bytes;
		let decoded: Vec<u32> = deserialize_indexed_seq(&mut r).unwrap();
		assert_eq!(decoded, original);
	}

	#[test]
	fn indexed_map_walker_finds_serialised_key() {
		let mut map: BTreeMap<u32, u32> = BTreeMap::new();
		for i in 0u32..16 {
			map.insert(i, i * 10);
		}
		let mut bytes = Vec::new();
		serialize_indexed_map(&map, &mut bytes).unwrap();

		let walker: IndexedMapWalker<u32, u32> = IndexedMapWalker::from_payload(&bytes).unwrap();

		// Serialise the target key the same way the encoder did.
		let target_key = 7u32;
		let mut target_bytes = Vec::new();
		target_key.serialize_revisioned(&mut target_bytes).unwrap();

		let value_bytes = walker
			.find_value_bytes(|k| k.cmp(target_bytes.as_slice()))
			.unwrap()
			.expect("key 7 should be present");
		// Decode the value.
		let mut r: &[u8] = value_bytes;
		use crate::DeserializeRevisioned;
		let v: u32 = u32::deserialize_revisioned(&mut r).unwrap();
		assert_eq!(v, 70);
	}
}
