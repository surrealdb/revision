mod common;
mod de;
mod reexport;
mod ser;
mod validate_version;

use std::u16;

use de::{DeserializeVisitor, EnumStructsVisitor};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use reexport::Reexport;
use ser::SerializeVisitor;
use validate_version::ValidateRevision;

use crate::ast::{self, Direct, ItemOptions, Visit};

pub fn revision(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
	let attrs: Direct<ItemOptions> = syn::parse2(attr)?;
	let ast: ast::Item = syn::parse2(input)?;

	let revision = match (ast.attrs.options.revision, attrs.0.revision) {
		(Some(x), None) | (None, Some(x)) => {
			x
		}
		(None,None) => {
			return Err(syn::Error::new(Span::call_site(),"Current revision not specified, please specify the current revision with `#[revisioned(revision = ..)]` "))
		}
		(Some(_),Some(_)) => {
			return Err(syn::Error::new(Span::call_site(),"Current revision specified twice"))
		}
	};

	if revision > u16::MAX as usize {
		return Err(syn::Error::new(
			Span::call_site(),
			format_args!("Revision exceeded maximum supported value of {}", u16::MAX),
		));
	}
	if revision == 0 {
		return Err(syn::Error::new(Span::call_site(), "Revision versions start at 1"));
	}

	// Make sure that all used revisions are less or equal to the current revision.
	ValidateRevision(revision).visit_item(&ast)?;

	// Recreate the item.
	let mut reexport = TokenStream::new();
	Reexport {
		revision,
		stream: &mut reexport,
	}
	.visit_item(&ast)
	.unwrap();

	// serialize implementation
	let mut serialize = TokenStream::new();
	SerializeVisitor::new(revision, &mut serialize).visit_item(&ast).unwrap();

	let mut deserialize_structs = TokenStream::new();
	EnumStructsVisitor::new(revision, &mut deserialize_structs).visit_item(&ast).unwrap();

	// deserialize implementation
	let deserialize = (1..=revision)
		.map(|x| {
			// one for every revision
			let mut deserialize = TokenStream::new();
			DeserializeVisitor {
				target: revision,
				current: x,
				stream: &mut deserialize,
			}
			.visit_item(&ast)
			.unwrap();

			let revision = x as u16;

			quote! {
				#revision => {
					#deserialize
				}
			}
		})
		.collect::<Vec<_>>();

	let name = match ast.kind {
		ast::ItemKind::Enum(x) => x.name,
		ast::ItemKind::Struct(x) => x.name,
	};
	let revision = revision as u16;
	let revision_error = format!("Invalid revision `{{}}` for type `{}`", name);

	Ok(quote! {
		#reexport
		#deserialize_structs

		impl ::revision::Revisioned for #name {
			fn revision() -> u16{
				#revision
			}

			fn serialize_revisioned<W: ::std::io::Write>(&self, writer: &mut W) -> ::std::result::Result<(), ::revision::Error> {
				::revision::Revisioned::serialize_revisioned(&Self::revision(),writer)?;
				#serialize
			}

			fn deserialize_revisioned<R: ::std::io::Read>(reader: &mut R) -> ::std::result::Result<Self, ::revision::Error> {
				let __revision = <u16 as ::revision::Revisioned>::deserialize_revisioned(reader)?;
				match __revision {
					#(#deserialize)*
					x => {
						return Err(::revision::Error::Deserialize(
							format!(#revision_error,x)
						))
					}
				}
			}
		}
	})
}
