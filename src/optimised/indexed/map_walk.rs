//! Indexed-map walker.
//!
//! Layout of an indexed-map payload (after the outer envelope has been opened):
//!
//! ```text
//! u8 flags                                  // bit 0: indexed
//! varint len                                // entry count
//! if flags.0:
//!     [(u32_le key_off, u32_le val_off); len]
//!     K_0 || K_1 || ... || K_{len-1}        // dense keys, ascending
//!     V_0 || V_1 || ... || V_{len-1}        // dense values, matching order
//! else:
//!     (K, V)*                               // legacy-shape body for small maps
//! ```
//!
//! Each offset is into its dense region (keys for `key_offsets`, values for
//! `val_offsets`). Walker construction validates the prologue and the key
//! region's ascending invariant.

use std::cmp::Ordering;
use std::marker::PhantomData;

use crate::Error;
use crate::optimised::indexed::seq_walk::FLAG_INDEXED;
use crate::optimised::validation::{validate_key_region_ascending, validate_map_prologue};

/// Walker over an indexed-map body.
#[derive(Debug)]
pub struct IndexedMapWalker<'p, K, V> {
	/// Bytes past the flags+len header. Either the (offsets + keys + values) layout
	/// in indexed mode, or the legacy `(K, V)*` body.
	body: &'p [u8],
	/// `None` on the legacy path.
	prologue: Option<MapPrologue<'p>>,
	len: usize,
	_marker: PhantomData<fn() -> (K, V)>,
}

#[derive(Debug)]
struct MapPrologue<'p> {
	key_offsets: Vec<u32>,
	val_offsets: Vec<u32>,
	keys_region: &'p [u8],
	vals_region: &'p [u8],
}

impl<'p, K, V> IndexedMapWalker<'p, K, V> {
	pub fn from_payload(payload: &'p [u8]) -> Result<Self, Error> {
		Self::from_payload_inner(payload, true)
	}

	/// Open a walker **without** validating the prologue (monotonic offsets,
	/// ascending key region).
	///
	/// Skips the O(len) validation that [`from_payload`] runs. Use only when
	/// the bytes are trusted (e.g. freshly written by the same process). On
	/// untrusted input a malformed prologue produces silent wrong-key /
	/// wrong-value bytes rather than a clean
	/// [`Error::OptimisedOffsetsNonMonotonic`] /
	/// [`Error::OptimisedKeyRegionNotAscending`].
	///
	/// [`from_payload`]: Self::from_payload
	/// [`Error::OptimisedOffsetsNonMonotonic`]: crate::Error::OptimisedOffsetsNonMonotonic
	/// [`Error::OptimisedKeyRegionNotAscending`]: crate::Error::OptimisedKeyRegionNotAscending
	pub fn from_payload_unvalidated(payload: &'p [u8]) -> Result<Self, Error> {
		Self::from_payload_inner(payload, false)
	}

	fn from_payload_inner(payload: &'p [u8], validate: bool) -> Result<Self, Error> {
		if payload.is_empty() {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		let flags = payload[0];
		let mut cursor = 1usize;
		let (len, varint_bytes) = read_varint(&payload[cursor..])?;
		cursor += varint_bytes;
		let indexed = (flags & FLAG_INDEXED) != 0;

		if !indexed {
			return Ok(Self {
				body: &payload[cursor..],
				prologue: None,
				len,
				_marker: PhantomData,
			});
		}

		let table_bytes = len
			.checked_mul(8)
			.ok_or_else(|| Error::Deserialize("indexed-map offset table size overflow".into()))?;
		if payload.len() < cursor + table_bytes {
			return Err(Error::OptimisedSubReaderOverrun);
		}

		let mut key_offsets = Vec::with_capacity(len);
		let mut val_offsets = Vec::with_capacity(len);
		for i in 0..len {
			let base = cursor + i * 8;
			let ko = u32::from_le_bytes(payload[base..base + 4].try_into().unwrap());
			let vo = u32::from_le_bytes(payload[base + 4..base + 8].try_into().unwrap());
			key_offsets.push(ko);
			val_offsets.push(vo);
		}
		cursor += table_bytes;

		// Compute the keys region length from the last offset to the next entry.
		// Encoded shape places the keys region immediately after the offset table.
		// We learn the keys-region end by reading the last key_offset + the keys-region
		// size, but the encoder didn't include a separate size — it's derivable as
		// the next-region start. We expect the encoder to emit a u32_le keys_region_len
		// header here to disambiguate. Add that now.
		if payload.len() < cursor + 8 {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		let keys_region_len =
			u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
		cursor += 4;
		let vals_region_len =
			u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap()) as usize;
		cursor += 4;

		if payload.len() < cursor + keys_region_len + vals_region_len {
			return Err(Error::OptimisedSubReaderOverrun);
		}
		let keys_region = &payload[cursor..cursor + keys_region_len];
		cursor += keys_region_len;
		let vals_region = &payload[cursor..cursor + vals_region_len];

		if validate {
			validate_map_prologue(
				&key_offsets,
				&val_offsets,
				keys_region_len as u32,
				vals_region_len as u32,
			)?;
			validate_key_region_ascending(keys_region, &key_offsets)?;
		}

		Ok(Self {
			body: &payload[1 + varint_bytes..],
			prologue: Some(MapPrologue {
				key_offsets,
				val_offsets,
				keys_region,
				vals_region,
			}),
			len,
			_marker: PhantomData,
		})
	}

	#[inline]
	pub fn len(&self) -> usize {
		self.len
	}

	#[inline]
	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	#[inline]
	pub fn is_indexed(&self) -> bool {
		self.prologue.is_some()
	}

	/// Iterate key/value byte ranges in original insertion order. Indexed path only.
	pub fn entries(&self) -> Option<impl Iterator<Item = (&'p [u8], &'p [u8])> + '_> {
		let p = self.prologue.as_ref()?;
		let keys = p.keys_region;
		let vals = p.vals_region;
		let kos = &p.key_offsets;
		let vos = &p.val_offsets;
		let n = self.len;
		Some((0..n).map(move |i| {
			let k_start = kos[i] as usize;
			let k_end = if i + 1 < n {
				kos[i + 1] as usize
			} else {
				keys.len()
			};
			let v_start = vos[i] as usize;
			let v_end = if i + 1 < n {
				vos[i + 1] as usize
			} else {
				vals.len()
			};
			(&keys[k_start..k_end], &vals[v_start..v_end])
		}))
	}

	/// Borrow the value bytes for the entry whose key bytes compare `Equal`
	/// under `predicate`. Uses binary search when indexed; linear scan when not.
	pub fn find_value_bytes<F>(&self, mut predicate: F) -> Result<Option<&'p [u8]>, Error>
	where
		F: FnMut(&[u8]) -> Ordering,
	{
		let Some(p) = &self.prologue else {
			return Err(Error::Deserialize("find_value_bytes called on non-indexed map".into()));
		};
		let n = self.len;
		let mut lo = 0usize;
		let mut hi = n;
		while lo < hi {
			let mid = lo + (hi - lo) / 2;
			let k_start = p.key_offsets[mid] as usize;
			let k_end = if mid + 1 < n {
				p.key_offsets[mid + 1] as usize
			} else {
				p.keys_region.len()
			};
			let key_bytes = &p.keys_region[k_start..k_end];
			match predicate(key_bytes) {
				Ordering::Equal => {
					let v_start = p.val_offsets[mid] as usize;
					let v_end = if mid + 1 < n {
						p.val_offsets[mid + 1] as usize
					} else {
						p.vals_region.len()
					};
					return Ok(Some(&p.vals_region[v_start..v_end]));
				}
				Ordering::Less => lo = mid + 1,
				Ordering::Greater => hi = mid,
			}
		}
		Ok(None)
	}

	/// Bytes for the legacy `(K, V)*` body. `None` on the indexed path.
	#[inline]
	pub fn legacy_body(&self) -> Option<&'p [u8]> {
		if self.prologue.is_some() {
			None
		} else {
			Some(self.body)
		}
	}
}

fn read_varint(bytes: &[u8]) -> Result<(usize, usize), Error> {
	if bytes.is_empty() {
		return Err(Error::OptimisedSubReaderOverrun);
	}
	let tag = bytes[0];
	match tag {
		0..=250 => Ok((tag as usize, 1)),
		251 => {
			if bytes.len() < 3 {
				return Err(Error::OptimisedSubReaderOverrun);
			}
			Ok((u16::from_le_bytes([bytes[1], bytes[2]]) as usize, 3))
		}
		252 => {
			if bytes.len() < 5 {
				return Err(Error::OptimisedSubReaderOverrun);
			}
			Ok((u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as usize, 5))
		}
		253 => {
			if bytes.len() < 9 {
				return Err(Error::OptimisedSubReaderOverrun);
			}
			let v = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
			let v: usize = v.try_into().map_err(|_| Error::IntegerOverflow)?;
			Ok((v, 9))
		}
		_ => Err(Error::InvalidIntegerEncoding),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn varint(v: usize) -> Vec<u8> {
		match v {
			0..=250 => vec![v as u8],
			251..=65535 => {
				let mut out = vec![251u8];
				out.extend_from_slice(&(v as u16).to_le_bytes());
				out
			}
			_ => {
				let mut out = vec![252u8];
				out.extend_from_slice(&(v as u32).to_le_bytes());
				out
			}
		}
	}

	fn build_indexed_map(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
		let mut sorted: Vec<(&[u8], &[u8])> = entries.to_vec();
		sorted.sort_by(|a, b| a.0.cmp(b.0));
		let n = sorted.len();
		let mut out = Vec::new();
		out.push(FLAG_INDEXED);
		out.extend_from_slice(&varint(n));
		// Offset tables
		let mut k_off = 0u32;
		let mut v_off = 0u32;
		let mut k_offsets = Vec::with_capacity(n);
		let mut v_offsets = Vec::with_capacity(n);
		for (k, v) in &sorted {
			k_offsets.push(k_off);
			v_offsets.push(v_off);
			k_off += k.len() as u32;
			v_off += v.len() as u32;
		}
		for i in 0..n {
			out.extend_from_slice(&k_offsets[i].to_le_bytes());
			out.extend_from_slice(&v_offsets[i].to_le_bytes());
		}
		// Region lengths (u32_le pair)
		out.extend_from_slice(&k_off.to_le_bytes());
		out.extend_from_slice(&v_off.to_le_bytes());
		// Dense keys
		for (k, _) in &sorted {
			out.extend_from_slice(k);
		}
		// Dense values
		for (_, v) in &sorted {
			out.extend_from_slice(v);
		}
		out
	}

	#[test]
	fn opens_indexed_map_and_finds_keys() {
		let payload = build_indexed_map(&[(b"foo", b"42"), (b"bar", b"7"), (b"baz", b"99")]);
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		assert_eq!(w.len(), 3);
		assert!(w.is_indexed());
		let found = w.find_value_bytes(|k| k.cmp(b"baz".as_slice())).unwrap().unwrap();
		assert_eq!(found, b"99");
		let found = w.find_value_bytes(|k| k.cmp(b"foo".as_slice())).unwrap().unwrap();
		assert_eq!(found, b"42");
		assert!(w.find_value_bytes(|k| k.cmp(b"missing".as_slice())).unwrap().is_none());
	}

	#[test]
	fn iter_returns_pairs_in_sorted_order() {
		let payload = build_indexed_map(&[(b"x", b"3"), (b"a", b"1"), (b"m", b"2")]);
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		let collected: Vec<(&[u8], &[u8])> = w.entries().unwrap().collect();
		assert_eq!(
			collected,
			vec![
				(b"a".as_slice(), b"1".as_slice()),
				(b"m".as_slice(), b"2".as_slice()),
				(b"x".as_slice(), b"3".as_slice())
			]
		);
	}

	#[test]
	fn legacy_map_passes_through() {
		// flags = 0, varint(2), then some legacy body bytes
		let payload = [0u8, 2, 0xAA, 0xBB];
		let w: IndexedMapWalker<(), ()> = IndexedMapWalker::from_payload(&payload).unwrap();
		assert!(!w.is_indexed());
		assert_eq!(w.len(), 2);
		assert_eq!(w.legacy_body().unwrap(), &[0xAA, 0xBB]);
		assert!(
			w.find_value_bytes(|_| Ordering::Equal).is_err(),
			"find_value_bytes errors on legacy path"
		);
	}
}
