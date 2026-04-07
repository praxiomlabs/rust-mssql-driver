//! `#[derive(FromRow)]` implementation.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type};

use crate::attributes::{parse_field_config, parse_struct_config};
use crate::naming::apply_rename_all;

pub(crate) fn impl_from_row(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let struct_config = parse_struct_config(&input.attrs);

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "FromRow can only be derived for structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "FromRow can only be derived for structs",
            ));
        }
    };

    let mut field_extractions = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let config = parse_field_config(&field.attrs);

        if config.skip {
            // Use Default for skipped fields
            field_extractions.push(quote! {
                #field_name: ::std::default::Default::default()
            });
            continue;
        }

        if config.flatten {
            // Recursively call FromRow for nested structs
            field_extractions.push(quote! {
                #field_name: <#field_type as mssql_client::FromRow>::from_row(row)?
            });
            continue;
        }

        // Determine the column name
        let column_name = config.rename.unwrap_or_else(|| {
            apply_rename_all(&field_name.to_string(), struct_config.rename_all.as_deref())
        });

        if config.default {
            // Use try_get_by_name which returns Option, fallback to Default
            if is_option_type(field_type) {
                field_extractions.push(quote! {
                    #field_name: row.try_get_by_name(#column_name)
                });
            } else {
                field_extractions.push(quote! {
                    #field_name: row.try_get_by_name(#column_name)
                        .unwrap_or_else(::std::default::Default::default)
                });
            }
        } else if is_option_type(field_type) {
            // Option types use try_get which handles NULL gracefully
            field_extractions.push(quote! {
                #field_name: row.try_get_by_name(#column_name)
            });
        } else {
            // Required fields use get_by_name which returns Result
            field_extractions.push(quote! {
                #field_name: row.get_by_name(#column_name)
                    .map_err(mssql_client::Error::from)?
            });
        }
    }

    Ok(quote! {
        impl #impl_generics mssql_client::FromRow for #name #ty_generics #where_clause {
            fn from_row(row: &mssql_client::Row) -> ::std::result::Result<Self, mssql_client::Error> {
                Ok(Self {
                    #(#field_extractions),*
                })
            }
        }
    })
}

/// Check if a type is an `Option<T>`.
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}
