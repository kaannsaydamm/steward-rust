//! Typed identifiers for Omega entities (Task 1.3 / §11).
//!
//! Each id is a newtype over String with serde roundtrip, Display, and
//! FromStr. SQLite conversions store the inner string.

use anyhow::{bail, Result};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

macro_rules! typed_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Generates a prefixed unique id (`<prefix>_<uuid>`).
            pub fn generate() -> Self {
                Self(format!(
                    "{}_{}",
                    $prefix,
                    uuid::Uuid::new_v4().simple()
                ))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(value: &str) -> Result<Self> {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!(concat!(stringify!($name), " cannot be empty"));
                }
                if trimmed.len() > 128 {
                    bail!(concat!(stringify!($name), " exceeds 128 characters"));
                }
                Ok(Self(trimmed.to_owned()))
            }
        }

        impl TryFrom<&str> for $name {
            type Error = anyhow::Error;
            fn try_from(value: &str) -> Result<Self> {
                value.parse()
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::from_str(&value).map_err(de::Error::custom)
            }
        }
    };
}

typed_id!(
    /// A project/repository/filesystem context.
    WorkspaceId,
    "ws"
);
typed_id!(
    /// Durable human-facing conversation/goal lineage.
    ThreadId,
    "thr"
);
typed_id!(
    /// One executable attempt/continuation under a thread.
    RunId,
    "run"
);
typed_id!(
    /// An execution unit in the IR.
    NodeId,
    "node"
);
typed_id!(
    /// One retry of a node.
    AttemptId,
    "att"
);
typed_id!(
    /// A concrete execution child with its own context/budget.
    AgentInstanceId,
    "agt"
);
typed_id!(
    /// Resumable state boundary.
    CheckpointId,
    "chk"
);
typed_id!(
    /// Persisted proof/output.
    ArtifactId,
    "art"
);
typed_id!(
    /// A durable human decision request.
    ApprovalId,
    "apr"
);
typed_id!(
    /// Reusable declarative behavior specification.
    AgentProfileId,
    "prf"
);

/// SQLite text conversion shared by stores.
pub mod sqlite {
    /// Binds any typed id as TEXT via its Display.
    pub fn text(id: &impl std::fmt::Display) -> String {
        id.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_empty_and_oversized() {
        assert!(ThreadId::from_str("").is_err());
        assert!(ThreadId::from_str("   ").is_err());
        assert!(ThreadId::from_str(&"x".repeat(129)).is_err());
        assert!(ThreadId::from_str("thr_abc").is_ok());
    }

    #[test]
    fn serde_roundtrip() {
        let run = RunId::new("run_123");
        let json = serde_json::to_string(&run).unwrap();
        assert_eq!(json, "\"run_123\"");
        let back: RunId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, run);
    }

    #[test]
    fn serde_rejects_invalid_on_deserialize() {
        let result = serde_json::from_str::<RunId>("\"\"");
        assert!(result.is_err());
    }

    #[test]
    fn generate_produces_prefixed_unique_ids() {
        let a = CheckpointId::generate();
        let b = CheckpointId::generate();
        assert!(a.as_str().starts_with("chk_"));
        assert!(b.as_str().starts_with("chk_"));
        assert_ne!(a, b);
    }

    #[test]
    fn display_matches_inner() {
        let approval = ApprovalId::new("apr_7");
        assert_eq!(approval.to_string(), "apr_7");
        assert_eq!(sqlite::text(&approval), "apr_7");
    }

    #[test]
    fn try_from_str_parses() {
        let node = NodeId::try_from("node_main").expect("valid");
        assert_eq!(node.as_str(), "node_main");
        assert!(NodeId::try_from("").is_err());
    }
}
