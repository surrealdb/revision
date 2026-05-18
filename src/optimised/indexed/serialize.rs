//! Encode side of indexed compounds.
//!
//! These helpers produce wire bytes that [`IndexedMapWalker`] and
//! [`IndexedSeqWalker`] expect on the read side. Each takes a target writer
//! and the data structure to serialise; the K, V, T types serialise their
//! components via `SerializeRevisioned`.
//!
//! [`IndexedMapWalker`]: crate::optimised::IndexedMapWalker
//! [`IndexedSeqWalker`]: crate::optimised::IndexedSeqWalker

use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, Hash};
use std::io::{Read, Write};
use std::marker::PhantomData;

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
/// revisions.
///
/// # Wire-format invariant (READ THIS BEFORE IMPLEMENTING)
///
/// The serialised keys region **must** be ascending under byte compare —
/// not under `K`'s [`Ord`] impl. The [`IndexedMapWalker`] binary-searches
/// the keys region by comparing raw bytes, so any divergence between
/// `K`-order and byte-order would silently produce wrong lookups (and is
/// caught at decode time by [`Error::OptimisedKeyRegionNotAscending`]).
///
/// This matters whenever `K`'s [`SerializeRevisioned`] impl emits a wire
/// prefix that is **not** monotone in `K`'s ordering. Concretely:
///
/// - `String`, `Box<str>`, `Vec<u8>`, `Bytes`, and any type whose
///   `SerializeRevisioned` is `varint(len) || bytes` — the varint length
///   breaks byte-order whenever keys have different lengths. For example:
///   `"delta"` (len 5) serialises to `[5, 'd', 'e', 'l', 't', 'a']` and
///   `"charlie"` (len 7) to `[7, 'c', 'h', ...]`; `"delta"` sorts **before**
///   `"charlie"` byte-wise but **after** under `String::cmp`.
/// - Most fixed-width primitive types (`u32`, `u64`, etc.) are also
///   problematic under varint encoding because small and large values get
///   different lengths.
/// - With `fixed-width-encoding` enabled, primitive keys are byte-monotone
///   under the natural integer order.
///
/// The supplied [`BTreeMap`] impl handles this correctly by pre-serialising
/// every entry and sorting the resulting `(key_bytes, val_bytes)` pairs by
/// `key_bytes` before writing — the same strategy any new impl should use.
///
/// # Round-trip preservation
///
/// Decode does **not** depend on the encode order: keys and values are
/// inserted into the target collection in the order they appear on the
/// wire, then re-sorted by `K::Ord` (for `BTreeMap`) or hashed (for
/// `HashMap`) on the receiving side. Encoding in byte-sorted order does
/// not change the deserialised value.
///
/// [`IndexedMapWalker`]: crate::optimised::IndexedMapWalker
/// [`Error::OptimisedKeyRegionNotAscending`]: crate::Error::OptimisedKeyRegionNotAscending
#[doc(hidden)]
pub trait IndexedMapEncoded: Sized {
	/// Key type, exposed so the walker codegen can name [`IndexedMapWalker`]'s
	/// type parameters from the field's encoded type.
	///
	/// [`IndexedMapWalker`]: crate::optimised::IndexedMapWalker
	type Key;
	/// Value type.
	type Value;
	fn serialize_indexed_map<W: Write>(&self, w: &mut W) -> Result<(), Error>;
	fn deserialize_indexed_map<R: Read>(r: &mut R) -> Result<Self, Error>;
	fn skip_indexed_map<R: Read>(r: &mut R) -> Result<(), Error>;
}

impl<K, V> IndexedMapEncoded for BTreeMap<K, V>
where
	K: SerializeRevisioned + DeserializeRevisioned + SkipRevisioned + Ord,
	V: SerializeRevisioned + DeserializeRevisioned + SkipRevisioned,
{
	type Key = K;
	type Value = V;
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

impl<K, V, S> IndexedMapEncoded for HashMap<K, V, S>
where
	K: SerializeRevisioned + DeserializeRevisioned + SkipRevisioned + Hash + Eq,
	V: SerializeRevisioned + DeserializeRevisioned + SkipRevisioned,
	S: BuildHasher + Default,
{
	type Key = K;
	type Value = V;
	fn serialize_indexed_map<W: Write>(&self, w: &mut W) -> Result<(), Error> {
		// `HashMap` iteration order is arbitrary, but
		// `serialize_indexed_entries` sorts entries by key bytes before
		// writing — exactly what the indexed wire format requires.
		serialize_indexed_entries(self.iter(), w)
	}
	fn deserialize_indexed_map<R: Read>(r: &mut R) -> Result<Self, Error> {
		// Mirror BTreeMap's deserializer but build a HashMap.
		let mut flag_buf = [0u8; 1];
		r.read_exact(&mut flag_buf).map_err(Error::Io)?;
		let flags = flag_buf[0];
		let len = read_varint(r)?;
		let mut out: HashMap<K, V, S> = HashMap::with_capacity_and_hasher(len, S::default());
		if (flags & FLAG_INDEXED) == 0 {
			for _ in 0..len {
				let k = K::deserialize_revisioned(r)?;
				let v = V::deserialize_revisioned(r)?;
				out.insert(k, v);
			}
			return Ok(out);
		}
		// Skip the offset tables + region lengths.
		let table_bytes = len.checked_mul(8).ok_or(Error::OptimisedSubReaderOverrun)?;
		let mut discard = vec![0u8; table_bytes + 8];
		r.read_exact(&mut discard).map_err(Error::Io)?;
		let mut keys: Vec<K> = Vec::with_capacity(len);
		for _ in 0..len {
			keys.push(K::deserialize_revisioned(r)?);
		}
		let mut values: Vec<V> = Vec::with_capacity(len);
		for _ in 0..len {
			values.push(V::deserialize_revisioned(r)?);
		}
		for (k, v) in keys.into_iter().zip(values) {
			out.insert(k, v);
		}
		Ok(out)
	}
	fn skip_indexed_map<R: Read>(r: &mut R) -> Result<(), Error> {
		skip_indexed_map::<K, V, R>(r)
	}
}

/// Sequence-shaped types under optimised. Implemented for [`Vec`].
#[doc(hidden)]
pub trait IndexedSeqEncoded: Sized {
	type Item;
	fn serialize_indexed_seq<W: Write>(&self, w: &mut W) -> Result<(), Error>;
	fn deserialize_indexed_seq<R: Read>(r: &mut R) -> Result<Self, Error>;
	fn skip_indexed_seq<R: Read>(r: &mut R) -> Result<(), Error>;
}

impl<T> IndexedSeqEncoded for Vec<T>
where
	T: SerializeRevisioned + DeserializeRevisioned + SkipRevisioned,
{
	type Item = T;
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

// -----------------------------------------------------------------------------
// Owned views (walker handles)
// -----------------------------------------------------------------------------

/// Owned wire-bytes handle for an indexed map field.
///
/// The walker's per-field `walk_<field>` / `into_walk_<field>` accessors
/// return one of these for `#[revision(indexed_map)]` fields. The view owns
/// the encoded payload as `Vec<u8>`; call [`walker`](Self::walker) to borrow
/// an [`IndexedMapWalker`] from it for binary-search lookups.
///
/// Lifetimes: the walker borrows from the view, so the view must outlive the
/// walker. Typical usage:
///
/// ```rust,ignore
/// let view = parent_walker.walk_fields()?;
/// let map_walker = view.walker()?;
/// let value_bytes = map_walker.find_value_bytes(|k| k.cmp(b"target"))?;
/// // value_bytes is valid until `view` is dropped.
/// ```
///
/// [`IndexedMapWalker`]: crate::optimised::IndexedMapWalker
pub struct OwnedIndexedMapView<K, V> {
	bytes: Vec<u8>,
	_marker: PhantomData<fn() -> (K, V)>,
}

impl<K, V> OwnedIndexedMapView<K, V> {
	#[doc(hidden)]
	pub fn new(bytes: Vec<u8>) -> Self {
		Self {
			bytes,
			_marker: PhantomData,
		}
	}

	/// Borrow an [`IndexedMapWalker`] over the owned wire bytes.
	///
	/// [`IndexedMapWalker`]: crate::optimised::IndexedMapWalker
	pub fn walker(&self) -> Result<crate::optimised::IndexedMapWalker<'_, K, V>, Error> {
		crate::optimised::IndexedMapWalker::from_payload(&self.bytes)
	}

	/// Raw wire bytes (for callers that want to feed them somewhere else).
	pub fn as_bytes(&self) -> &[u8] {
		&self.bytes
	}

	/// Consume and return the owned bytes.
	pub fn into_bytes(self) -> Vec<u8> {
		self.bytes
	}
}

/// Owned wire-bytes handle for an indexed sequence field. Mirror of
/// [`OwnedIndexedMapView`] for the sequence case.
pub struct OwnedIndexedSeqView<T> {
	bytes: Vec<u8>,
	_marker: PhantomData<fn() -> T>,
}

impl<T> OwnedIndexedSeqView<T> {
	#[doc(hidden)]
	pub fn new(bytes: Vec<u8>) -> Self {
		Self {
			bytes,
			_marker: PhantomData,
		}
	}

	pub fn walker(&self) -> Result<crate::optimised::IndexedSeqWalker<'_, T>, Error> {
		crate::optimised::IndexedSeqWalker::from_payload(&self.bytes)
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.bytes
	}

	pub fn into_bytes(self) -> Vec<u8> {
		self.bytes
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
/// Convenience wrapper: serialise any `BTreeMap` using the indexed wire format.
///
/// Equivalent to [`serialize_indexed_entries`] over `map.iter()`. Provided as
/// a stable entry point for hand-written `IndexedMapEncoded` impls.
#[doc(hidden)]
pub fn serialize_indexed_map<K, V, W: Write>(
	map: &BTreeMap<K, V>,
	writer: &mut W,
) -> Result<(), Error>
where
	K: SerializeRevisioned,
	V: SerializeRevisioned,
{
	serialize_indexed_entries(map.iter(), writer)
}

/// Serialise an iterator of `(&K, &V)` pairs using the indexed wire format.
///
/// Use this directly when implementing [`IndexedMapEncoded`] for map types
/// that are not [`BTreeMap`] (e.g. `HashMap`, `imbl::OrdMap`, custom map
/// types). The function pre-serialises every entry, sorts the pairs by
/// `key_bytes` (so the dense keys region is byte-ascending — see
/// [`IndexedMapEncoded`] for why this matters), then writes the wire
/// shape.
///
/// `len` is taken from `IntoIterator::Item` consumption; callers should
/// pass a `&Map` whose iterator yields each entry exactly once.
#[doc(hidden)]
pub fn serialize_indexed_entries<'a, I, K, V, W>(entries: I, writer: &mut W) -> Result<(), Error>
where
	I: IntoIterator<Item = (&'a K, &'a V)>,
	K: SerializeRevisioned + 'a,
	V: SerializeRevisioned + 'a,
	W: Write,
{
	// Pre-serialise each entry so we know the offsets and region sizes.
	// IMPORTANT: callers may pass entries in any order — for hash-based maps
	// the iterator order is arbitrary, and even for sorted maps the K-order
	// may diverge from the byte-order of the serialised keys. We therefore
	// sort the pre-serialised entries by key bytes before writing.
	let mut pairs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
	for (k, v) in entries {
		let mut kb = Vec::new();
		k.serialize_revisioned(&mut kb)?;
		let mut vb = Vec::new();
		v.serialize_revisioned(&mut vb)?;
		pairs.push((kb, vb));
	}
	pairs.sort_by(|a, b| a.0.cmp(&b.0));
	let len = pairs.len();
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

/// Convenience wrapper: serialise a `&[T]` slice using the indexed wire format.
///
/// Equivalent to [`serialize_indexed_seq_iter`] over `items.iter()`. Provided
/// as a stable entry point for hand-written `IndexedSeqEncoded` impls on
/// `Vec`-like types.
#[doc(hidden)]
pub fn serialize_indexed_seq<T, W: Write>(items: &[T], writer: &mut W) -> Result<(), Error>
where
	T: SerializeRevisioned,
{
	serialize_indexed_seq_iter(items.iter(), writer)
}

/// Serialise an iterator of `&T` items using the indexed wire format.
///
/// Use this directly when implementing [`IndexedSeqEncoded`] for sequence
/// types that are not `Vec` (e.g. `imbl::Vector`, custom seq types).
///
/// Wire layout:
///
/// ```text
/// u8 flags                       // bit 0: indexed
/// varint len                     // element count
/// [u32_le elem_off; len]         // offset table
/// elements concatenated
/// ```
#[doc(hidden)]
pub fn serialize_indexed_seq_iter<'a, I, T, W>(items: I, writer: &mut W) -> Result<(), Error>
where
	I: IntoIterator<Item = &'a T>,
	T: SerializeRevisioned + 'a,
	W: Write,
{
	let mut bodies: Vec<Vec<u8>> = Vec::new();
	for item in items {
		let mut b = Vec::new();
		item.serialize_revisioned(&mut b)?;
		bodies.push(b);
	}
	let len = bodies.len();

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
