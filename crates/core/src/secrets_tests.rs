#![allow(unused_imports)]
use super::{resolve, EnvSecretStore, OsSecretStore, SecretRef, SecretStore};

#[test]
fn secret_ref_formats_and_validates() {
    let reference = SecretRef::new("provider", "openai");
    assert_eq!(reference.to_string(), "steward://secret/provider/openai");
    assert_eq!(reference.entry_name().unwrap(), "provider/openai");
    assert!(SecretRef("invalid".into()).entry_name().is_err());
}

#[test]
fn env_store_reads_from_process_environment() {
    let store = EnvSecretStore::new();
    let key = SecretRef::new("provider", "test-key");
    let variable = "STEWARD_SECRET_PROVIDER_TEST_KEY";
    std::env::set_var(variable, "value-123");

    assert_eq!(store.get(&key).unwrap().as_deref(), Some("value-123"));
    assert!(!store.is_secure());

    // put is refused: env fallback cannot persist.
    assert!(store.put(&key, "x").is_err());
    std::env::remove_var(variable);
}

#[test]
fn env_store_missing_entry_is_none() {
    let store = EnvSecretStore::new();
    let key = SecretRef::new("provider", "definitely-missing-key");
    assert_eq!(store.get(&key).unwrap(), None);
}

#[cfg(target_os = "windows")]
#[test]
fn os_store_roundtrips_through_credential_manager() {
    let store = OsSecretStore::new();
    let key = SecretRef::new("test-scope", "omega-roundtrip");
    // Clean any residue from an aborted prior run.
    let _ = store.delete(&key);

    assert_eq!(store.get(&key).unwrap(), None);
    store.put(&key, "hunter2").unwrap();
    assert_eq!(store.get(&key).unwrap().as_deref(), Some("hunter2"));
    store.delete(&key).unwrap();
    assert_eq!(store.get(&key).unwrap(), None);
    assert!(store.is_secure());
}

#[cfg(target_os = "windows")]
#[test]
fn resolve_prefers_os_vault_when_probe_succeeds() {
    let store = resolve(false).expect("resolve secret store");
    assert!(store.is_secure(), "expected OS vault on Windows CI/desktop");
}

#[test]
fn secret_ref_never_carries_value() {
    // The reference format has no way to embed a value; entry name is scope/name.
    let reference = SecretRef::new("provider", "anthropic");
    let entry = reference.entry_name().unwrap();
    assert!(!entry.contains('=') && !entry.contains(' '));
}
