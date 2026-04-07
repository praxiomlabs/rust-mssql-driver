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
use syn::parse_macro_input;

mod attributes;
mod from_row;
mod naming;
mod to_params;
mod tvp;

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
    let input = parse_macro_input!(input as syn::DeriveInput);
    match from_row::impl_from_row(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
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
    let input = parse_macro_input!(input as syn::DeriveInput);
    match to_params::impl_to_params(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
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
    let input = parse_macro_input!(input as syn::DeriveInput);
    match tvp::impl_tvp(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
