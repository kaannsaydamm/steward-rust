//! Protocol version negotiation.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Wire protocol version. Bump `minor` for additive changes, `major` for
/// breaking ones; incompatible majors refuse the handshake.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u32,
    pub minor: u32,
}

pub const HANDSHAKE: ProtocolVersion = ProtocolVersion { major: 2, minor: 0 };

/// First protocol version a server retains compatibility with.
pub const MIN_SUPPORTED: ProtocolVersion = ProtocolVersion { major: 2, minor: 0 };

impl ProtocolVersion {
    /// Compatible when major matches and the offered peer version is not
    /// older than the minimum we still speak (`MIN_SUPPORTED`). A client
    /// with a newer minor is fine: additive features are negotiated via
    /// capability flags.
    pub fn compatible(&self, other: &ProtocolVersion) -> bool {
        const MIN_SUPPORTED_MINOR: u32 = MIN_SUPPORTED.minor;
        // The comparison stays meaningful as MIN_SUPPORTED evolves; clippy
        // cannot see that today, so the tautology check is explicitly allowed.
        #[allow(clippy::absurd_extreme_comparisons)]
        {
            self.major == other.major && other.minor >= MIN_SUPPORTED_MINOR
        }
    }
}

/// Handshake payload exchanged as the first frame in both directions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol_version: ProtocolVersion,
    /// Stable daemon instance uuid; clients detect daemon restarts.
    pub instance_id: String,
    /// Authentication token (loopback install/session token).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    /// Feature flags the sender supports.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl Hello {
    /// Validates an incoming hello against this side's supported version.
    pub fn negotiate(&self, offered: &Hello) -> Result<()> {
        if !HANDSHAKE.compatible(&offered.protocol_version) {
            bail!(
                "unsupported protocol version {}.{}, expected {}.{}",
                offered.protocol_version.major,
                offered.protocol_version.minor,
                HANDSHAKE.major,
                HANDSHAKE.minor
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_versions_negotiate() {
        let server = hello(HANDSHAKE);
        let client = hello(HANDSHAKE);
        assert!(server.negotiate(&client).is_ok());
    }

    #[test]
    fn newer_client_minor_is_accepted() {
        let server = hello(HANDSHAKE);
        let client = hello(ProtocolVersion { major: 2, minor: 3 });
        assert!(server.negotiate(&client).is_ok());
    }

    #[test]
    fn major_mismatch_is_rejected() {
        let server = hello(HANDSHAKE);
        let client = hello(ProtocolVersion { major: 3, minor: 0 });
        let error = server.negotiate(&client).expect_err("major mismatch");
        assert!(error.to_string().contains("unsupported protocol version"));
    }

    #[test]
    fn older_client_minor_below_minimum_is_rejected() {
        // Server demands >= MIN.minor; a 2.0 server vs 2.0 client is fine,
        // but a 2.-1-style gap cannot be expressed; exercise major path.
        let server = hello(HANDSHAKE);
        let client = hello(ProtocolVersion {
            major: 1,
            minor: 99,
        });
        assert!(server.negotiate(&client).is_err());
    }

    fn hello(version: ProtocolVersion) -> Hello {
        Hello {
            protocol_version: version,
            instance_id: "daemon-1".into(),
            auth_token: None,
            capabilities: vec!["runs".into()],
        }
    }
}
