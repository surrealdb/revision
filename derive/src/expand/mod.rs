mod common;
mod de;
mod reexport;
mod ser;
mod validate_version;

use de::{DeserializeVisitor, EnumStructsVisitor};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Token, WhereClause};
use syn::punctuated::Punctuated;
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

    let (name, generics) = match &ast.kind {
        ast::ItemKind::Enum(x) => (&x.name, &x.generics),
        ast::ItemKind::Struct(x) => (&x.name, &x.generics),
    };

    let mut serialise_where_clause = if let Some(where_clause) = generics.where_clause.as_ref() {
        where_clause.clone()
    } else {
        WhereClause {
            where_token: <Token![where]>::default(),
            predicates: Punctuated::new(),
        }
    };

    let mut deserialise_where_clause = if let Some(where_clause) = generics.where_clause.as_ref() {
        where_clause.clone()
    } else {
        WhereClause {
            where_token: <Token![where]>::default(),
            predicates: Punctuated::new(),
        }
    };

    let mut types = vec![];

    for ty in generics.type_params() {
        let span = ty.span();

        serialise_where_clause.predicates.push(syn::parse_quote_spanned!{span=>
            #ty: ::revision::SerializeRevisioned
        });
        deserialise_where_clause.predicates.push(syn::parse_quote_spanned!{span=>
            #ty: ::revision::DeserializeRevisioned
        });

        types.push(ty.ident.clone());
    }

	// serialize implementation
	let mut serialize = TokenStream::new();
	SerializeVisitor::new(revision, &mut serialize).visit_item(&ast).unwrap();

	let mut deserialize_structs = TokenStream::new();
	EnumStructsVisitor::new(revision, types, &mut deserialize_structs).visit_item(&ast).unwrap();

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

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	let revision = revision as u16;
	let revision_error = format!("Invalid revision `{{}}` for type `{}`", name);

	let serialize_impl = if attrs.0.serialize {
		quote! {
			impl #impl_generics ::revision::SerializeRevisioned for #name #ty_generics #serialise_where_clause {
				fn serialize_revisioned<W: ::std::io::Write>(&self, writer: &mut W) -> ::std::result::Result<(), ::revision::Error> {
					::revision::SerializeRevisioned::serialize_revisioned(&<Self as ::revision::Revisioned>::revision(),writer)?;
					#serialize
				}
			}
		}
	} else {
		quote! {}
	};

	let deserialize_impl = if attrs.0.deserialize {
		quote! {
			impl #impl_generics ::revision::DeserializeRevisioned for #name #ty_generics #deserialise_where_clause {
				fn deserialize_revisioned<R: ::std::io::Read>(reader: &mut R) -> ::std::result::Result<Self, ::revision::Error> {
					let __revision = <u16 as ::revision::DeserializeRevisioned>::deserialize_revisioned(reader)?;
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
		}
	} else {
		quote! {}
	};

	Ok(quote! {
		#reexport

        const _: () = {
    		#deserialize_structs

            #serialize_impl
            #deserialize_impl

            impl #impl_generics ::revision::Revisioned for #name #ty_generics #where_clause {
                #[inline]
                fn revision() -> u16{
                    #revision
                }
            }
        };

	})
}
