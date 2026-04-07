//! Attribute parsing for `#[mssql(...)]` derive macro configuration.

use syn::{Attribute, Expr, ExprLit, Lit};

/// Field configuration extracted from `#[mssql(...)]` attributes.
#[derive(Default)]
pub(crate) struct FieldConfig {
    /// Renamed column/parameter name.
    pub rename: Option<String>,
    /// Skip this field.
    pub skip: bool,
    /// Use default value if missing.
    pub default: bool,
    /// Flatten nested struct.
    pub flatten: bool,
}

/// Struct-level configuration extracted from `#[mssql(...)]` attributes.
#[derive(Default)]
pub(crate) struct StructConfig {
    /// TVP type name.
    pub type_name: Option<String>,
    /// Rename all fields using a casing convention.
    pub rename_all: Option<String>,
}

/// Parse field-level `#[mssql(...)]` attributes.
pub(crate) fn parse_field_config(attrs: &[Attribute]) -> FieldConfig {
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

/// Parse struct-level `#[mssql(...)]` attributes.
pub(crate) fn parse_struct_config(attrs: &[Attribute]) -> StructConfig {
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
