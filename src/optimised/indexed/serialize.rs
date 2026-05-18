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
use std::io::Write;

use crate::Error;
use crate::SerializeRevisioned;
use crate::optimised::indexed::seq_walk::FLAG_INDEXED;

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
	let mut keys: Vec<Vec<u8>> = Vec::with_capacity(len);
	let mut vals: Vec<Vec<u8>> = Vec::with_capacity(len);
	for (k, v) in map.iter() {
		let mut kb = Vec::new();
		k.serialize_revisioned(&mut kb)?;
		keys.push(kb);
		let mut vb = Vec::new();
		v.serialize_revisioned(&mut vb)?;
		vals.push(vb);
	}

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
