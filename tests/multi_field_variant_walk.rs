//! Multi-field optimised enum variants expose `<variant>_view` for streaming
//! descent into the variant body. The view holds borrowed bytes from the
//! parent walker's source — zero copy — and the caller decodes the multi
//! fields sequentially.

use revision::prelude::*;

#[revisioned(revision(1, encoding = "optimised"))]
#[derive(Debug, PartialEq)]
enum Value {
	#[revision(size = "inline")]
	Null,
	#[revision(size = "varlen")]
	Tup(u32, String),
	#[revision(size = "varlen")]
	Named {
		id: u32,
		label: String,
	},
}

#[test]
fn multi_field_tuple_variant_view_borrows_body() {
	let v = Value::Tup(42, "hello".into());
	let bytes = revision::to_vec(&v).unwrap();

	let mut r: &[u8] = &bytes;
	let w = Value::walk_revisioned(&mut r).unwrap();
	let view = w.tup_view().unwrap();

	// View borrows from the source `bytes`.
	let body = view.as_bytes();
	assert!(
		body.as_ptr() >= bytes.as_ptr()
			&& body.as_ptr() <= unsafe { bytes.as_ptr().add(bytes.len()) },
		"view's bytes must lie inside source buffer (no copy)"
	);

	// Decode the two fields sequentially from the body.
	let mut cursor: &[u8] = body;
	let id = <u32 as DeserializeRevisioned>::deserialize_revisioned(&mut cursor).unwrap();
	let label = <String as DeserializeRevisioned>::deserialize_revisioned(&mut cursor).unwrap();
	assert_eq!(id, 42);
	assert_eq!(label, "hello");
}

#[test]
fn multi_field_struct_variant_view_borrows_body() {
	let v = Value::Named {
		id: 7,
		label: "world".into(),
	};
	let bytes = revision::to_vec(&v).unwrap();

	let mut r: &[u8] = &bytes;
	let w = Value::walk_revisioned(&mut r).unwrap();
	let view = w.named_view().unwrap();

	let body = view.as_bytes();
	assert!(
		body.as_ptr() >= bytes.as_ptr()
			&& body.as_ptr() <= unsafe { bytes.as_ptr().add(bytes.len()) }
	);

	let mut cursor: &[u8] = body;
	let id = <u32 as DeserializeRevisioned>::deserialize_revisioned(&mut cursor).unwrap();
	let label = <String as DeserializeRevisioned>::deserialize_revisioned(&mut cursor).unwrap();
	assert_eq!(id, 7);
	assert_eq!(label, "world");
}

#[test]
fn multi_field_view_errors_on_wrong_variant() {
	let v = Value::Null;
	let bytes = revision::to_vec(&v).unwrap();
	let mut r: &[u8] = &bytes;
	let w = Value::walk_revisioned(&mut r).unwrap();
	let err = match w.tup_view() {
		Ok(_) => panic!("Null is not Tup"),
		Err(e) => e,
	};
	match err {
		revision::Error::Deserialize(msg) => {
			assert!(msg.contains("variant mismatch"), "got: {msg}");
		}
		other => panic!("expected Deserialize, got {other:?}"),
	}
}
