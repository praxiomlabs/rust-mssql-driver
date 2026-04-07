//! Case conversion utilities for field name renaming.

/// Convert a field name to a column name based on the `rename_all` setting.
pub(crate) fn apply_rename_all(name: &str, rename_all: Option<&str>) -> String {
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
