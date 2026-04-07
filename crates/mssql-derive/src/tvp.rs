//! `#[derive(Tvp)]` implementation for table-valued parameters.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type};

use crate::attributes::{parse_field_config, parse_struct_config};
use crate::naming::apply_rename_all;

pub(crate) fn impl_tvp(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let struct_config = parse_struct_config(&input.attrs);

    let type_name = struct_config.type_name.ok_or_else(|| {
        syn::Error::new_spanned(
            input,
            "Tvp derive requires #[mssql(type_name = \"schema.TypeName\")] attribute",
        )
    })?;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "Tvp can only be derived for structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "Tvp can only be derived for structs",
            ));
        }
    };

    let mut column_defs = Vec::new();
    let mut value_extractions = Vec::new();
    let mut ordinal = 0usize;

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let config = parse_field_config(&field.attrs);

        if config.skip {
            continue;
        }

        // Determine the column name
        let column_name = config.rename.unwrap_or_else(|| {
            apply_rename_all(&field_name.to_string(), struct_config.rename_all.as_deref())
        });

        // Infer SQL type from Rust type
        let sql_type = infer_sql_type(field_type);

        column_defs.push(quote! {
            mssql_client::TvpColumn::new(#column_name, #sql_type, #ordinal)
        });

        value_extractions.push(quote! {
            mssql_types::ToSql::to_sql(&self.#field_name)?
        });

        ordinal += 1;
    }

    Ok(quote! {
        impl #impl_generics mssql_client::Tvp for #name #ty_generics #where_clause {
            fn type_name() -> &'static str {
                #type_name
            }

            fn columns() -> ::std::vec::Vec<mssql_client::TvpColumn> {
                ::std::vec![
                    #(#column_defs),*
                ]
            }

            fn to_row(&self) -> ::std::result::Result<mssql_client::TvpRow, mssql_types::TypeError> {
                Ok(mssql_client::TvpRow::new(::std::vec![
                    #(#value_extractions),*
                ]))
            }
        }
    })
}

/// Infer SQL type string from Rust type.
fn infer_sql_type(ty: &Type) -> &'static str {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = segment.ident.to_string();
            return match type_name.as_str() {
                "i8" | "u8" => "TINYINT",
                "i16" => "SMALLINT",
                "i32" => "INT",
                "i64" => "BIGINT",
                "f32" => "REAL",
                "f64" => "FLOAT",
                "bool" => "BIT",
                "String" => "NVARCHAR(MAX)",
                "Uuid" => "UNIQUEIDENTIFIER",
                "NaiveDate" => "DATE",
                "NaiveTime" => "TIME",
                "NaiveDateTime" => "DATETIME2",
                "DateTime" => "DATETIMEOFFSET",
                "Decimal" => "DECIMAL(38,10)",
                "Vec" => "VARBINARY(MAX)",
                "Option" => {
                    // For Option<T>, try to get the inner type
                    // This is a simplified approach
                    "NVARCHAR(MAX)"
                }
                _ => "NVARCHAR(MAX)",
            };
        }
    }
    "NVARCHAR(MAX)"
}
