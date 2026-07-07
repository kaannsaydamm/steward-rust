use super::{needs_setup, save_provider_profile, Settings};
use std::fs;
use steward_core::provider_config::{ProviderProfile, ProviderProtocol, ProviderSettings};

#[test]
fn setup_is_required_until_valid_settings_are_saved() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let path = temp.path().join("setup.json");

    assert!(needs_setup(&path));

    let settings = Settings::new(3100).expect("valid port");
    settings.save(&path).expect("save settings");

    assert!(!needs_setup(&path));
    assert_eq!(Settings::load(&path).expect("load settings"), settings);
}

#[test]
fn invalid_or_incomplete_settings_require_setup() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let path = temp.path().join("setup.json");
    fs::write(&path, br#"{"version":1,"web_port":0}"#).expect("write invalid settings");

    assert!(needs_setup(&path));
    assert!(Settings::new(0).is_err());
    assert!(Settings::new(50051).is_err());
}

#[test]
fn settings_expose_local_service_addresses() {
    let settings = Settings::new(3000).expect("valid port");

    assert_eq!(settings.daemon_url(), "http://127.0.0.1:50051");
    assert_eq!(settings.web_url(), "http://127.0.0.1:3000");
}

#[test]
fn provider_profile_is_saved_and_activated() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("providers.json");
    let profile = ProviderProfile {
        profile_id: "default".to_owned(),
        provider_id: "lm-studio".to_owned(),
        display_name: "LM Studio".to_owned(),
        protocol: ProviderProtocol::OpenAiChat,
        base_url: "http://127.0.0.1:1234/v1".to_owned(),
        model: "local-model".to_owned(),
        api_key_env: None,
        api_key: None,
    };

    save_provider_profile(&path, profile).expect("save provider profile");

    let settings = ProviderSettings::load(&path).expect("load provider settings");
    assert_eq!(
        settings.active().expect("active provider").provider_id,
        "lm-studio"
    );
}
