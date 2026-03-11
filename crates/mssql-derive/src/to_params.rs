//! `#[derive(ToParams)]` implementation.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::attributes::{parse_field_config, parse_struct_config};
use crate::naming::apply_rename_all;

pub(crate) fn impl_to_params(input: &DeriveInput) -> syn::Result<TokenStream2> {
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
                    "ToParams can only be derived for structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "ToParams can only be derived for structs",
            ));
        }
    };

    let mut param_creations = Vec::new();
    let mut field_count = 0usize;

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let config = parse_field_config(&field.attrs);

        if config.skip {
            continue;
        }

        field_count += 1;

        // Determine the parameter name
        let param_name = config.rename.unwrap_or_else(|| {
            apply_rename_all(&field_name.to_string(), struct_config.rename_all.as_deref())
        });

        param_creations.push(quote! {
            mssql_client::NamedParam::from_value(#param_name, &self.#field_name)?
        });
    }

    Ok(quote! {
        impl #impl_generics mssql_client::ToParams for #name #ty_generics #where_clause {
            fn to_params(&self) -> ::std::result::Result<
                ::std::vec::Vec<mssql_client::NamedParam>,
                mssql_types::TypeError
            > {
                Ok(::std::vec![
                    #(#param_creations),*
                ])
            }

            fn param_count(&self) -> ::std::option::Option<usize> {
                ::std::option::Option::Some(#field_count)
            }
        }
    })
}
