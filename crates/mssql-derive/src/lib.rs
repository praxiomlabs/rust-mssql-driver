// Proc macros operate on named structs where field.ident is always Some
#![allow(clippy::unwrap_used)]

//! # mssql-derive
//!
//! Procedural macros for SQL Server row mapping and parameter handling.
//!
//! This crate provides derive macros for automatically implementing
//! row-to-struct mapping and struct-to-parameter conversion.
//!
//! ## Available Macros
//!
//! - `#[derive(FromRow)]` - Convert database rows to structs
//! - `#[derive(ToParams)]` - Convert structs to query parameters
//! - `#[derive(Tvp)]` - Table-valued parameter support
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_derive::{FromRow, ToParams};
//!
//! // Automatic row mapping
//! #[derive(FromRow)]
//! struct User {
//!     id: i32,
//!     #[mssql(rename = "user_name")]
//!     name: String,
//!     email: Option<String>,
//! }
//!
//! // Automatic parameter conversion
//! #[derive(ToParams)]
//! struct NewUser {
//!     name: String,
//!     email: String,
//! }
//! ```

#![warn(missing_docs)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Expr, ExprLit, Fields, Lit, Type, parse_macro_input};

/// Field configuration extracted from attributes.
#[derive(Default)]
struct FieldConfig {
    /// Renamed column/parameter name.
    rename: Option<String>,
    /// Skip this field.
    skip: bool,
    /// Use default value if missing.
    default: bool,
    /// Flatten nested struct.
    flatten: bool,
}

/// Struct-level configuration extracted from attributes.
#[derive(Default)]
struct StructConfig {
    /// TVP type name.
    type_name: Option<String>,
    /// Rename all fields using a casing convention.
    rename_all: Option<String>,
}

/// Parse mssql attributes from a list of attributes.
fn parse_field_config(attrs: &[Attribute]) -> FieldConfig {
    let mut config = FieldConfig::default();

    for attr in attrs {
        if !attr.path().is_ident("mssql") {
            continue;
        }

        // Parse the attribute using syn 2.0 API
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let value: Expr = meta.value()?.parse()?;
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(lit), ..
                }) = value
                {
                    config.rename = Some(lit.value());
                }
            } else if meta.path.is_ident("skip") {
                config.skip = true;
            } else if meta.path.is_ident("default") {
                config.default = true;
            } else if meta.path.is_ident("flatten") {
                config.flatten = true;
            }
            Ok(())
        });
    }

    config
}

/// Parse struct-level mssql attributes.
fn parse_struct_config(attrs: &[Attribute]) -> StructConfig {
    let mut config = StructConfig::default();

    for attr in attrs {
        if !attr.path().is_ident("mssql") {
            continue;
        }

        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("type_name") {
                let value: Expr = meta.value()?.parse()?;
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(lit), ..
                }) = value
                {
                    config.type_name = Some(lit.value());
                }
            } else if meta.path.is_ident("rename_all") {
                let value: Expr = meta.value()?.parse()?;
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(lit), ..
                }) = value
                {
                    config.rename_all = Some(lit.value());
                }
            }
            Ok(())
        });
    }

    config
}

/// Convert a field name to a column name based on rename_all setting.
fn apply_rename_all(name: &str, rename_all: Option<&str>) -> String {
    match rename_all {
        Some("snake_case") => to_snake_case(name),
        Some("camelCase") => to_camel_case(name),
        Some("PascalCase") => to_pascal_case(name),
        Some("SCREAMING_SNAKE_CASE") => to_screaming_snake_case(name),
        _ => name.to_string(),
    }
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.extend(c.to_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else if i == 0 {
            result.extend(c.to_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

fn to_screaming_snake_case(s: &str) -> String {
    to_snake_case(s).to_uppercase()
}

/// Derive macro for implementing `FromRow` trait.
///
/// This macro generates code to convert a database row into a struct.
///
/// ## Attributes
///
/// ### Field Attributes
///
/// - `#[mssql(rename = "column_name")]` - Map field to a different column name
/// - `#[mssql(skip)]` - Skip this field (must have a Default implementation)
/// - `#[mssql(default)]` - Use Default if column is NULL or missing
/// - `#[mssql(flatten)]` - Flatten a nested struct implementing FromRow
///
/// ### Struct Attributes
///
/// - `#[mssql(rename_all = "snake_case")]` - Apply naming convention to all fields
///
/// ## Example
///
/// ```rust,ignore
/// #[derive(FromRow)]
/// #[mssql(rename_all = "PascalCase")]
/// struct User {
///     id: i32,
///     #[mssql(rename = "UserName")]
///     name: String,
///     #[mssql(default)]
///     email: Option<String>,
///     #[mssql(skip)]
///     computed: String,
/// }
/// ```
#[proc_macro_derive(FromRow, attributes(mssql))]
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_from_row(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn impl_from_row(input: &DeriveInput) -> syn::Result<TokenStream2> {
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

/// Check if a type is an Option<T>.
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

/// Derive macro for implementing `ToParams` trait.
///
/// This macro generates code to convert a struct into query parameters.
///
/// ## Attributes
///
/// - `#[mssql(rename = "param_name")]` - Use a different parameter name
/// - `#[mssql(skip)]` - Don't include this field as a parameter
///
/// ## Example
///
/// ```rust,ignore
/// #[derive(ToParams)]
/// struct NewUser {
///     name: String,
///     #[mssql(rename = "email_address")]
///     email: String,
///     #[mssql(skip)]
///     internal_id: u64,
/// }
///
/// let user = NewUser {
///     name: "Alice".into(),
///     email: "alice@example.com".into(),
///     internal_id: 0,
/// };
///
/// client.execute(
///     "INSERT INTO users (name, email_address) VALUES (@name, @email_address)",
///     &user.to_params()?,
/// ).await?;
/// ```
#[proc_macro_derive(ToParams, attributes(mssql))]
pub fn derive_to_params(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_to_params(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn impl_to_params(input: &DeriveInput) -> syn::Result<TokenStream2> {
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

/// Derive macro for implementing `Tvp` trait (Table-Valued Parameters).
///
/// This macro generates code to use a struct as a table-valued parameter row.
///
/// ## Attributes
///
/// ### Struct Attributes (Required)
///
/// - `#[mssql(type_name = "schema.TypeName")]` - SQL Server TVP type name
///
/// ### Field Attributes
///
/// - `#[mssql(rename = "column_name")]` - Map field to a different column name
/// - `#[mssql(skip)]` - Don't include this field in the TVP
///
/// ## Example
///
/// First, create the table type in SQL Server:
///
/// ```sql
/// CREATE TYPE dbo.UserIdList AS TABLE (
///     UserId INT NOT NULL
/// );
/// ```
///
/// Then derive the trait:
///
/// ```rust,ignore
/// #[derive(Tvp)]
/// #[mssql(type_name = "dbo.UserIdList")]
/// struct UserId {
///     #[mssql(rename = "UserId")]
///     user_id: i32,
/// }
///
/// let ids = vec![UserId { user_id: 1 }, UserId { user_id: 2 }];
/// let tvp = TvpValue::new(&ids)?;
/// ```
#[proc_macro_derive(Tvp, attributes(mssql))]
pub fn derive_tvp(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_tvp(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn impl_tvp(input: &DeriveInput) -> syn::Result<TokenStream2> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("userName"), "user_name");
        assert_eq!(to_snake_case("UserName"), "user_name");
        assert_eq!(to_snake_case("user_name"), "user_name");
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("user_name"), "userName");
        assert_eq!(to_camel_case("UserName"), "userName");
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("user_name"), "UserName");
        assert_eq!(to_pascal_case("userName"), "UserName");
    }

    #[test]
    fn test_to_screaming_snake_case() {
        assert_eq!(to_screaming_snake_case("userName"), "USER_NAME");
        assert_eq!(to_screaming_snake_case("user_name"), "USER_NAME");
    }
}
