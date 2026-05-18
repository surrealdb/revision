//! Optimised codegen for revisioned structs.
//!
//! Wire layout under `encoding = "optimised"`:
//!
//! ```text
//! u16 revision                            (written by the outer impl)
//! u32_le payload_length                   (this module)
//! [optional indexed prologue]             when `struct = "indexed"`
//! field_0 || field_1 || ... || field_{n-1}
//! ```
//!
//! The encoder writes the payload into a scratch `Vec<u8>` to learn its length
//! before flushing it to the outer writer, mirroring the runtime crate's
//! `encode_varlen` strategy.

use proc_macro2::TokenStream;
use quote::{TokenStreamExt, quote};

use crate::ast::{Field, Fields, Struct};

use super::super::context::EncodingContext;

/// Emit the optimised serialize body for a struct.
///
/// `self.<field>` bindings are produced first (matching the legacy serialize
/// visitor's pattern), then the payload is buffered into `__scratch` and the
/// u32_le length is written, then the bytes.
pub fn emit_struct_serialize(s: &Struct, ctx: EncodingContext) -> TokenStream {
	let revision = ctx.revision as usize;
	let mut out = TokenStream::new();
	let alive_fields: Vec<&Field> = alive_fields(s, revision);

	// Bind each alive field to a local matching the legacy visitor.
	for (idx, f) in alive_fields.iter().enumerate() {
		let binding = f.name.to_binding();
		match f.colon_token {
			Some(_) => {
				let name = &f.name;
				out.append_all(quote! { let #binding = &self.#name; });
			}
			None => {
				let idx_ident = syn::Index {
					index: idx as u32,
					span: proc_macro2::Span::call_site(),
				};
				out.append_all(quote! { let #binding = &self.#idx_ident; });
			}
		}
	}

	let indexed = ctx.struct_is_indexed();
	let field_count = alive_fields.len();
	let prologue_bytes = if indexed {
		field_count * 4
	} else {
		0
	};

	// Sequential or indexed: build into a scratch buffer and flush.
	out.append_all(quote! {
		let mut __scratch: ::std::vec::Vec<u8> = ::std::vec::Vec::new();
	});

	if indexed {
		// Reserve `field_count * 4` bytes for offsets; we'll patch them after the
		// fields are written.
		out.append_all(quote! {
			let __prologue_start: usize = __scratch.len();
			__scratch.resize(__prologue_start + #prologue_bytes, 0u8);
		});
	}

	// Capture offsets as we write each field.
	for (idx, f) in alive_fields.iter().enumerate() {
		let binding = f.name.to_binding();
		if indexed {
			out.append_all(quote! {
				let __off_pos = __prologue_start + (#idx * 4);
				let __off = (__scratch.len() - __prologue_start) as u32;
				let __off_bytes = __off.to_le_bytes();
				__scratch[__off_pos..__off_pos + 4].copy_from_slice(&__off_bytes);
			});
		}
		out.append_all(quote! {
			::revision::SerializeRevisioned::serialize_revisioned(#binding, &mut __scratch)?;
		});
		let _ = f; // satisfy clippy when binding is unused (e.g. unit struct edge case)
	}

	out.append_all(quote! {
		let __len: u32 = __scratch.len()
			.try_into()
			.map_err(|_| ::revision::Error::Serialize(
				"optimised struct payload exceeds u32::MAX bytes".into()
			))?;
		::std::io::Write::write_all(writer, &__len.to_le_bytes())
			.map_err(::revision::Error::Io)?;
		::std::io::Write::write_all(writer, &__scratch)
			.map_err(::revision::Error::Io)?;
		Ok(())
	});

	out
}

/// Emit the optimised deserialize body for a struct.
///
/// `target` is the latest revision (which the runtime type matches);
/// `current` (== `ctx.revision`) is the wire revision being decoded.
pub fn emit_struct_deserialize(s: &Struct, ctx: EncodingContext, target: usize) -> TokenStream {
	let current = ctx.revision as usize;
	let indexed = ctx.struct_is_indexed();
	let alive_at_current = alive_fields(s, current);
	let field_count = alive_at_current.len();
	let prologue_bytes = if indexed {
		field_count * 4
	} else {
		0
	};

	let mut decode_each = TokenStream::new();
	let mut bindings_for_construction: Vec<proc_macro2::TokenStream> = Vec::new();

	// First handle fields that exist at `target`:
	// - if they also exist at `current`, decode from wire
	// - if absent at `current`, synthesize via default_fn or Default
	let all_fields = collect_fields(s);
	for f in &all_fields {
		let exists_current = f.attrs.options.exists_at(current);
		let exists_target = f.attrs.options.exists_at(target);
		let binding = f.name.to_binding();
		let ty = &f.ty;

		if exists_current && exists_target {
			decode_each.append_all(quote! {
				let #binding = <#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __payload)?;
			});
			bindings_for_construction.push(quote! { #binding });
		} else if !exists_current && exists_target {
			// Field added later — synthesize.
			let default = if let Some(default_fn) = &f.attrs.options.default {
				let ident = syn::Ident::new(&default_fn.value(), default_fn.span());
				let rev_lit = current as u16;
				quote! { Self::#ident(#rev_lit)? }
			} else {
				quote! { ::std::default::Default::default() }
			};
			decode_each.append_all(quote! {
				let #binding: #ty = #default;
			});
			bindings_for_construction.push(quote! { #binding });
		} else if exists_current && !exists_target {
			// Field on wire but removed in current type — decode and pass to convert_fn.
			let convert = f
				.attrs
				.options
				.convert
				.as_ref()
				.expect("convert_fn required when `end` is set; checked by AST validation");
			let convert_ident = syn::Ident::new(&convert.value(), convert.span());
			let rev_lit = current as u16;
			decode_each.append_all(quote! {
				let #binding = <#ty as ::revision::DeserializeRevisioned>::deserialize_revisioned(&mut __payload)?;
			});
			// Apply convert_fn after construction (handled below via __post_construct).
			let _ = (convert_ident, rev_lit);
		}
		// !exists_current && !exists_target: nothing to do at this revision.
	}

	let construct = match s.fields {
		Fields::Named {
			..
		} => quote! { let mut __this = Self { #(#bindings_for_construction),* }; },
		Fields::Unnamed {
			..
		} => quote! { let mut __this = Self ( #(#bindings_for_construction),* ); },
		Fields::Unit => quote! { let __this = Self; },
	};

	// After construction, apply convert_fn for fields present-on-wire but removed-in-target.
	let mut post_construct = TokenStream::new();
	for f in &all_fields {
		let exists_current = f.attrs.options.exists_at(current);
		let exists_target = f.attrs.options.exists_at(target);
		if exists_current && !exists_target {
			let binding = f.name.to_binding();
			let convert = f.attrs.options.convert.as_ref().unwrap();
			let convert_ident = syn::Ident::new(&convert.value(), convert.span());
			let rev_lit = current as u16;
			post_construct.append_all(quote! {
				Self::#convert_ident(&mut __this, #rev_lit, #binding)?;
			});
		}
	}

	let prologue_skip = if indexed {
		quote! {
			// Skip past the offset table — sequential decode doesn't need it.
			if __payload.len() < #prologue_bytes {
				return Err(::revision::Error::OptimisedSubReaderOverrun);
			}
			__payload = &__payload[#prologue_bytes..];
		}
	} else {
		quote! {}
	};

	quote! {
		let mut __byte_len_buf = [0u8; 4];
		::std::io::Read::read_exact(reader, &mut __byte_len_buf)
			.map_err(::revision::Error::Io)?;
		let __byte_len = u32::from_le_bytes(__byte_len_buf) as usize;
		let mut __payload_buf: ::std::vec::Vec<u8> = ::std::vec![0u8; __byte_len];
		::std::io::Read::read_exact(reader, &mut __payload_buf)
			.map_err(::revision::Error::Io)?;
		let mut __payload: &[u8] = &__payload_buf;
		#prologue_skip
		#decode_each
		#construct
		#post_construct
		Ok(__this)
	}
}

/// Emit the optimised skip body for a struct: read the u32_le length and advance.
pub fn emit_struct_skip(_s: &Struct, _ctx: EncodingContext, slice_mode: bool) -> TokenStream {
	if slice_mode {
		quote! {
			let mut __byte_len_buf = [0u8; 4];
			::std::io::Read::read_exact(reader, &mut __byte_len_buf)
				.map_err(::revision::Error::Io)?;
			let __byte_len = u32::from_le_bytes(__byte_len_buf) as usize;
			reader.consume(__byte_len)?;
			Ok(())
		}
	} else {
		quote! {
			let mut __byte_len_buf = [0u8; 4];
			::std::io::Read::read_exact(reader, &mut __byte_len_buf)
				.map_err(::revision::Error::Io)?;
			let __byte_len = u32::from_le_bytes(__byte_len_buf) as usize;
			::revision::slice_reader::advance_read(reader, __byte_len)?;
			Ok(())
		}
	}
}

fn collect_fields(s: &Struct) -> Vec<&Field> {
	match &s.fields {
		Fields::Named {
			fields,
			..
		}
		| Fields::Unnamed {
			fields,
			..
		} => fields.iter().collect(),
		Fields::Unit => Vec::new(),
	}
}

fn alive_fields(s: &Struct, revision: usize) -> Vec<&Field> {
	collect_fields(s).into_iter().filter(|f| f.attrs.options.exists_at(revision)).collect()
}
