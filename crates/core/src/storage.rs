use std::path::PathBuf;

pub fn root_from(
    override_path: Option<&str>,
    user_profile: Option<&str>,
    home: Option<&str>,
) -> Option<PathBuf> {
    override_path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            user_profile
                .filter(|path| !path.trim().is_empty())
                .or_else(|| home.filter(|path| !path.trim().is_empty()))
                .map(|path| PathBuf::from(path).join(".steward"))
        })
}

pub fn root() -> Option<PathBuf> {
    let override_path = std::env::var("STEWARD_HOME").ok();
    let user_profile = std::env::var("USERPROFILE").ok();
    let home = std::env::var("HOME").ok();
    root_from(
        override_path.as_deref(),
        user_profile.as_deref(),
        home.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::root_from;
    use std::path::PathBuf;

    #[test]
    fn explicit_override_wins_over_platform_home() {
        assert_eq!(
            root_from(Some("D:/portable"), Some("C:/Users/operator"), None),
            Some(PathBuf::from("D:/portable"))
        );
    }

    #[test]
    fn defaults_to_hidden_steward_directory_in_user_home() {
        assert_eq!(
            root_from(None, Some("C:/Users/operator"), None),
            Some(PathBuf::from("C:/Users/operator").join(".steward"))
        );
        assert_eq!(
            root_from(None, None, Some("/home/operator")),
            Some(PathBuf::from("/home/operator").join(".steward"))
        );
    }
}
