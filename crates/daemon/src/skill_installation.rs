use crate::tool_registry::{self, SkillDefinition};
use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillManifest {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub tool_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillBundle {
    pub manifest_json: String,
    pub publisher_key: String,
    pub signature: String,
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("skill bundle signature is invalid")]
    InvalidSignature,
    #[error("skill manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("skill requires unknown tool '{0}'")]
    UnknownTool(String),
    #[error(transparent)]
    Storage(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Clock(#[from] std::time::SystemTimeError),
    #[error(transparent)]
    Registry(#[from] anyhow::Error),
}

pub fn decode_bundle(bytes: &[u8]) -> Result<SkillBundle, InstallError> {
    serde_json::from_slice(bytes).map_err(Into::into)
}

pub fn install(
    connection: &Connection,
    bundle: &SkillBundle,
) -> Result<SkillDefinition, InstallError> {
    verify_signature(bundle)?;
    let manifest: SkillManifest = serde_json::from_str(&bundle.manifest_json)?;
    validate_manifest(&manifest)?;
    let installed_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
    let manifest_digest = hex::encode(Sha256::digest(bundle.manifest_json.as_bytes()));
    let transaction = connection.unchecked_transaction()?;
    for tool_id in &manifest.tool_ids {
        if tool_registry::get_tool(&transaction, tool_id)?.is_none() {
            return Err(InstallError::UnknownTool(tool_id.clone()));
        }
    }
    transaction.execute(
        "INSERT INTO skills
         (skill_id, name, description, version, enabled, publisher_key,
          signature, manifest_digest, installed_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8)
         ON CONFLICT(skill_id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            version = excluded.version,
            enabled = 1,
            publisher_key = excluded.publisher_key,
            signature = excluded.signature,
            manifest_digest = excluded.manifest_digest,
            installed_at = excluded.installed_at",
        params![
            manifest.skill_id,
            manifest.name,
            manifest.description,
            manifest.version,
            bundle.publisher_key,
            bundle.signature,
            manifest_digest,
            installed_at
        ],
    )?;
    transaction.execute(
        "DELETE FROM skill_tools WHERE skill_id = ?1",
        [&manifest.skill_id],
    )?;
    for (position, tool_id) in manifest.tool_ids.iter().enumerate() {
        transaction.execute(
            "INSERT INTO skill_tools (skill_id, tool_id, position, required)
             VALUES (?1, ?2, ?3, 1)",
            params![manifest.skill_id, tool_id, position],
        )?;
    }
    transaction.commit()?;
    tool_registry::get_skill(connection, &manifest.skill_id)?.ok_or_else(|| {
        InstallError::InvalidManifest("installed skill could not be loaded".to_owned())
    })
}

fn verify_signature(bundle: &SkillBundle) -> Result<(), InstallError> {
    let key_bytes =
        hex::decode(&bundle.publisher_key).map_err(|_| InstallError::InvalidSignature)?;
    let key_bytes: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| InstallError::InvalidSignature)?;
    let signature_bytes =
        hex::decode(&bundle.signature).map_err(|_| InstallError::InvalidSignature)?;
    let signature_bytes: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| InstallError::InvalidSignature)?;
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| InstallError::InvalidSignature)?;
    key.verify(
        bundle.manifest_json.as_bytes(),
        &Signature::from_bytes(&signature_bytes),
    )
    .map_err(|_| InstallError::InvalidSignature)
}

fn validate_manifest(manifest: &SkillManifest) -> Result<(), InstallError> {
    if manifest.skill_id.trim().is_empty()
        || manifest.name.trim().is_empty()
        || manifest.description.trim().is_empty()
        || manifest.version.trim().is_empty()
        || manifest.tool_ids.is_empty()
    {
        return Err(InstallError::InvalidManifest(
            "id, name, description, version, and tools are required".to_owned(),
        ));
    }
    let mut unique_tools = HashSet::new();
    if manifest
        .tool_ids
        .iter()
        .any(|tool_id| tool_id.trim().is_empty() || !unique_tools.insert(tool_id))
    {
        return Err(InstallError::InvalidManifest(
            "tool ids must be non-empty and unique".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "skill_installation_tests.rs"]
mod tests;
