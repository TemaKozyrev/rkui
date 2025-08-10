/// Utilities for parsing security-related configs.
/// Non-business logic isolated from the Kafka service.

/// Try to extract username and password from a JAAS-like config string.
/// Expected patterns include username="user" password="pass" (quotes can be ' or ").
pub(crate) fn parse_username_password_from_jaas(s: &str) -> Option<(String, String)> {
    fn extract(field: &str, s: &str) -> Option<String> {
        let needle = format!("{}=", field);
        let idx = s.find(&needle)?;
        let after = &s[idx + needle.len()..];
        match after.chars().next()? {
            '"' | '\'' => { /* handled below */ }
            _ => {
                // Unquoted value: read until whitespace or semicolon
                let end = after.find(|ch: char| ch.is_whitespace() || ch == ';').unwrap_or(after.len());
                return Some(after[..end].to_string());
            }
        }
        let first = after.chars().next()?;
        if first == '"' || first == '\'' {
            let q = first;
            let rest = &after[first.len_utf8()..];
            if let Some(end) = rest.find(q) {
                return Some(rest[..end].to_string());
            }
        }
        None
    }
    let user = extract("username", s)?;
    let pass = extract("password", s)?;
    Some((user, pass))
}
