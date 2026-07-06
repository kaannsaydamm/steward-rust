const PLACEHOLDER: &str = "[REDACTED]";

const KNOWN_PREFIXES: &[&str] = &[
    "sk-ant-",
    "sk-proj-",
    "sk-",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "xoxs-",
    "AKIA",
    "ASIA",
    "AIzaSy",
];

const SENSITIVE_KEY_MARKERS: &[&str] = &["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "PWD"];

/// Redacts secret-shaped values (API keys, tokens, `KEY=value` assignments) from text
/// before it is persisted to the audit log or sent back to the model as tool output.
pub fn redact(text: &str) -> String {
    text.split_inclusive(char::is_whitespace)
        .map(redact_token)
        .collect()
}

fn redact_token(token: &str) -> String {
    let (word, trailing) = split_trailing_whitespace(token);
    if word.is_empty() {
        return token.to_owned();
    }
    if let Some((key, value)) = word.split_once('=') {
        if !value.is_empty() && looks_sensitive(key) {
            return format!("{key}={PLACEHOLDER}{trailing}");
        }
    }
    if starts_with_known_prefix(word) || is_pem_block_marker(word) {
        return format!("{PLACEHOLDER}{trailing}");
    }
    token.to_owned()
}

fn split_trailing_whitespace(token: &str) -> (&str, &str) {
    let trim_start = token.trim_end_matches(char::is_whitespace).len();
    (&token[..trim_start], &token[trim_start..])
}

fn looks_sensitive(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    SENSITIVE_KEY_MARKERS
        .iter()
        .any(|marker| upper.contains(marker))
}

fn starts_with_known_prefix(word: &str) -> bool {
    KNOWN_PREFIXES.iter().any(|prefix| word.starts_with(prefix))
}

fn is_pem_block_marker(word: &str) -> bool {
    word.starts_with("-----BEGIN") || word.starts_with("-----END")
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn redacts_known_provider_key_prefixes() {
        let input = "here is a key sk-ant-api03-abc123XYZ and also ghp_1234567890";
        let output = redact(input);
        assert!(!output.contains("sk-ant-api03-abc123XYZ"));
        assert!(!output.contains("ghp_1234567890"));
        assert!(output.contains("here is a key"));
    }

    #[test]
    fn redacts_key_value_assignments_for_sensitive_names() {
        let input = "export AWS_SECRET_ACCESS_KEY=abcd1234EFGH\nPATH=/usr/bin";
        let output = redact(input);
        assert!(!output.contains("abcd1234EFGH"));
        assert!(output.contains("AWS_SECRET_ACCESS_KEY=[REDACTED]"));
        assert!(output.contains("PATH=/usr/bin"));
    }

    #[test]
    fn leaves_ordinary_text_untouched() {
        let input = "the build finished with 0 errors and 3 warnings";
        assert_eq!(redact(input), input);
    }

    #[test]
    fn redacts_pem_private_key_markers() {
        let input = "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----";
        let output = redact(input);
        assert!(!output.contains("-----BEGIN PRIVATE KEY-----"));
    }
}
