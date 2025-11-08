use heck::{
    ToKebabCase, ToLowerCamelCase, ToPascalCase, ToShoutySnakeCase, ToSnakeCase, ToTitleCase,
};
use unicode_ident::is_xid_continue;

#[derive(Debug)]
pub enum Case {
    Pascal,
    Snake,
    ShoutySnake,
    Camel,
    Title,
    Kebab,
    Lowercase,
}
impl Case {
    pub fn sanitize(&self, input: &str) -> String {
        // Early return for empty strings to avoid unnecessary processing
        if input.is_empty() {
            return String::new();
        }
        let to_case = match self {
            Case::Pascal => str::to_pascal_case,
            Case::Snake => str::to_snake_case,
            Case::ShoutySnake => str::to_shouty_snake_case,
            Case::Camel => str::to_lower_camel_case,
            Case::Title => str::to_title_case,
            Case::Kebab => str::to_kebab_case,
            Case::Lowercase => {
                |s: &str| s.to_lowercase().replace(|c: char| !is_xid_continue(c), "")
            }
        };

        let mut cased = to_case(input);
        // allow leading & trailing underscores
        if input.starts_with("_") && !cased.starts_with("_") {
            cased = format!("_{cased}")
        }
        if input.ends_with("_") && !cased.ends_with("_") {
            cased = format!("{cased}_")
        }

        cased
    }
}

#[cfg(test)]
mod test {
    use super::Case;

    #[test]
    fn test_trailing_underscore() {
        let input = "ident_";
        assert_eq!(Case::Camel.sanitize(input), "ident_");
    }

    #[test]
    fn test_leading_underscore() {
        let input = "_ident";
        assert_eq!(Case::Camel.sanitize(input), "_ident");
    }
}
