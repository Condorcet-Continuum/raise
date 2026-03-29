pub struct StringUtils;

impl StringUtils {
    pub fn to_snake_case(input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut last_was_underscore = true; // Évite l'underscore au début

        for c in input.chars() {
            if c.is_uppercase() {
                if !last_was_underscore && !result.is_empty() {
                    result.push('_');
                }
                result.push(c.to_ascii_lowercase());
                last_was_underscore = false;
            } else if c == ' ' || c == '-' || c == '_' {
                if !last_was_underscore {
                    result.push('_');
                    last_was_underscore = true;
                }
            } else {
                result.push(c);
                last_was_underscore = false;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case_math() {
        assert_eq!(StringUtils::to_snake_case("PascalCase"), "pascal_case");
        assert_eq!(StringUtils::to_snake_case("Space Case"), "space_case"); // ✅ Fix double underscore
        assert_eq!(
            StringUtils::to_snake_case("snake_case_remains"),
            "snake_case_remains"
        );
    }
}
