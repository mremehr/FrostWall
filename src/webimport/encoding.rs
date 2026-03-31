pub(super) fn encode(value: &str) -> String {
    let mut result = String::new();
    for character in value.chars() {
        match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                result.push(character);
            }
            ' ' => result.push_str("%20"),
            _ => {
                for byte in character.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    result
}
