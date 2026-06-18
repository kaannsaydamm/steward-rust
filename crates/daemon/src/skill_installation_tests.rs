use super::{install, InstallError, SkillBundle, SkillManifest};
use crate::tool_registry::{self, list_skills};
use ed25519_dalek::{Signer as _, SigningKey};
use rusqlite::Connection;

const TEST_SECRET: [u8; 32] = [7; 32];

fn signed_bundle(manifest: &SkillManifest) -> SkillBundle {
    let signing_key = SigningKey::from_bytes(&TEST_SECRET);
    let manifest_json = serde_json::to_string(manifest).expect("serialize manifest");
    let signature = signing_key.sign(manifest_json.as_bytes());
    SkillBundle {
        manifest_json,
        publisher_key: hex::encode(signing_key.verifying_key().as_bytes()),
        signature: hex::encode(signature.to_bytes()),
    }
}

fn manifest(tool_ids: &[&str]) -> SkillManifest {
    SkillManifest {
        skill_id: "signed-research".to_owned(),
        name: "Signed research".to_owned(),
        description: "Research with a verified publisher".to_owned(),
        version: "1.2.0".to_owned(),
        tool_ids: tool_ids.iter().map(|id| (*id).to_owned()).collect(),
    }
}

#[test]
fn installs_self_signed_bundle_when_signature_is_valid() {
    let connection = Connection::open_in_memory().expect("open database");
    tool_registry::initialize(&connection).expect("initialize registry");
    let bundle = signed_bundle(&manifest(&["memory.recall"]));

    let installed = install(&connection, &bundle).expect("install signed skill");

    assert_eq!(installed.id, "signed-research");
    assert!(installed.enabled);
    assert_eq!(installed.publisher_key, bundle.publisher_key);
    assert!(installed.installed_at > 0.0);
}

#[test]
fn rejects_bundle_when_manifest_changes_after_signing() {
    let connection = Connection::open_in_memory().expect("open database");
    tool_registry::initialize(&connection).expect("initialize registry");
    let mut bundle = signed_bundle(&manifest(&["memory.recall"]));
    bundle.manifest_json = bundle.manifest_json.replace("1.2.0", "9.9.9");

    let error = install(&connection, &bundle).expect_err("reject tampered bundle");

    assert!(matches!(error, InstallError::InvalidSignature));
    assert_eq!(list_skills(&connection).expect("list skills").len(), 3);
}

#[test]
fn rejects_bundle_atomically_when_required_tool_is_unknown() {
    let connection = Connection::open_in_memory().expect("open database");
    tool_registry::initialize(&connection).expect("initialize registry");
    let bundle = signed_bundle(&manifest(&["missing.tool"]));

    let error = install(&connection, &bundle).expect_err("reject missing tool");

    assert!(matches!(error, InstallError::UnknownTool(tool) if tool == "missing.tool"));
    assert_eq!(list_skills(&connection).expect("list skills").len(), 3);
}
