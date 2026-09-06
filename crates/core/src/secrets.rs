//! OS-backed secret storage for provider credentials (Omega Task 1.1).
//!
//! Secrets live in the platform credential vault (Windows Credential Manager,
//! macOS Keychain, Linux Secret Service). Config files store only references;
//! when no vault service is available, an environment-only fallback keeps
//! Steward usable without silently degrading security posture.

use anyhow::{Context, Result};
use std::fmt;

/// Reference stored in config in place of the raw secret.
/// Format: `steward://secret/<scope>/<name>`.

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SecretRef(pub String);

impl SecretRef {
    pub fn new(scope: &str, name: &str) -> Self {
        Self(format!("steward://secret/{scope}/{name}"))
    }

    /// Vault entry name derived from the reference (never contains the value).
    pub fn entry_name(&self) -> Result<&str> {
        self.0
            .strip_prefix("steward://secret/")
            .filter(|rest| !rest.is_empty())
            .context("secret reference must start with steward://secret/")
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Platform secret vault.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &SecretRef) -> Result<Option<String>>;
    fn put(&self, key: &SecretRef, value: &str) -> Result<()>;
    fn delete(&self, key: &SecretRef) -> Result<()>;
    /// Whether this backend is the real OS vault (false = env fallback).
    fn is_secure(&self) -> bool;
}

/// OS credential vault (Windows Credential Manager / Keychain / Secret Service).
pub struct OsSecretStore;

impl OsSecretStore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OsSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for OsSecretStore {
    fn get(&self, key: &SecretRef) -> Result<Option<String>> {
        let entry = keyring::Entry::new("steward", key.entry_name()?)
            .context("opening keyring entry")?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error @ keyring::Error::NoStorageAccess { .. })
            | Err(error @ keyring::Error::PlatformFailure(_)) => {
                // Vault exists but refused: fail loudly, never silently fall back.
                Err(anyhow::Error::new(error).context("reading secret from OS vault"))
            }
            Err(error) => Err(anyhow::Error::new(error).context("reading secret from OS vault")),
        }
    }

    fn put(&self, key: &SecretRef, value: &str) -> Result<()> {
        let entry = keyring::Entry::new("steward", key.entry_name()?)
            .context("opening keyring entry")?;
        match entry.set_password(value) {
            Ok(()) => Ok(()),
            Err(
                error @ keyring::Error::NoStorageAccess { .. }
                | error @ keyring::Error::PlatformFailure(_),
            ) => Err(anyhow::Error::new(error).context("writing secret to OS vault")),
            Err(error) => Err(anyhow::Error::new(error).context("writing secret to OS vault")),
        }
    }

    fn delete(&self, key: &SecretRef) -> Result<()> {
        let entry = keyring::Entry::new("steward", key.entry_name()?)
            .context("opening keyring entry")?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(anyhow::Error::new(error).context("deleting secret from OS vault")),
        }
    }

    fn is_secure(&self) -> bool {
        true
    }
}

/// Development/CI fallback: reads secrets from the process environment only.
/// `put` is a no-op error and `get` looks up `STEWARD_SECRET_<SCOPE>_<NAME>`.
pub struct EnvSecretStore;

impl EnvSecretStore {
    pub fn new() -> Self {
        Self
    }

    fn env_name(key: &SecretRef) -> Result<String> {
        let rest = key.entry_name()?;
        let name: String = rest
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' })
            .collect();
        Ok(format!("STEWARD_SECRET_{name}"))
    }
}

impl Default for EnvSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for EnvSecretStore {
    fn get(&self, key: &SecretRef) -> Result<Option<String>> {
        let name = Self::env_name(key)?;
        Ok(std::env::var(&name).ok().filter(|v| !v.is_empty()))
    }

    fn put(&self, key: &SecretRef, _value: &str) -> Result<()> {
        anyhow::bail!(
            "environment fallback cannot persist secrets; set {} instead",
            Self::env_name(key)?
        );
    }

    fn delete(&self, _key: &SecretRef) -> Result<()> {
        Ok(())
    }

    fn is_secure(&self) -> bool {
        false
    }
}

/// Resolves the best available backend. `require_secure` forces the OS vault;
/// callers that can degrade use `false` and surface `is_secure()` in the UI.
pub fn resolve(require_secure: bool) -> Result<Box<dyn SecretStore>> {
    if require_secure {
        return Ok(Box::new(OsSecretStore::new()));
    }
    if env_vault_probe().is_ok() {
        Ok(Box::new(OsSecretStore::new()))
    } else {
        Ok(Box::new(EnvSecretStore::new()))
    }
}

fn env_vault_probe() -> Result<()> {
    let probe = SecretRef::new("probe", "vault-probe");
    let store = OsSecretStore::new();
    let marker = "steward-vault-probe";
    store.put(&probe, marker)?;
    let read_back = store.get(&probe)?;
    let _ = store.delete(&probe);
    match read_back {
        Some(value) if value == marker => Ok(()),
        other => anyhow::bail!("vault probe roundtrip mismatch: {other:?}"),
    }
}

#[path = "secrets_tests.rs"]
mod tests;
