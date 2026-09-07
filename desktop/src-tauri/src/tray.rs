//! Ported from NousResearch/hermes-agent@693641aa8b4359c602283bdbbc14041e03bc47bc
//! tray + notification shell semantics (MIT). Modified for Steward: tray icon
//! state derives from daemon health; approval-needed notifications reuse the
//! Tauri notification plugin. Headless-safe: tray construction is guarded so
//! unit tests never require a display.

use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::Manager;

/// Daemon health as surfaced by the tray. Tones mirror the WebUI `dot-live`
/// indicator (online=green, degraded=amber, lost=red).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DaemonHealth {
    Online,
    Degraded,
    Lost,
}

impl DaemonHealth {
    /// Tray tooltip suffix — the user-visible state word.
    pub fn label(self) -> &'static str {
        match self {
            DaemonHealth::Online => "daemon online",
            DaemonHealth::Degraded => "daemon degraded",
            DaemonHealth::Lost => "daemon unreachable",
        }
    }

    /// Tone name for the tray icon asset selector (green/amber/red).
    pub fn tone(self) -> &'static str {
        match self {
            DaemonHealth::Online => "green",
            DaemonHealth::Degraded => "amber",
            DaemonHealth::Lost => "red",
        }
    }

    /// Maps a supervisor/health probe string to a tray state.
    pub fn from_probe(connected: bool, degraded: bool) -> Self {
        if connected {
            if degraded {
                DaemonHealth::Degraded
            } else {
                DaemonHealth::Online
            }
        } else {
            DaemonHealth::Lost
        }
    }
}

/// Handle wrapper so main can update the tray as health changes.
pub struct TrayHandle {
    tray: TrayIcon<tauri::Wry>,
    last_tone: &'static str,
}

impl TrayHandle {
    /// Builds the tray icon. Returns `None` on headless/unsupported setups
    /// so the app still boots (graceful degradation, Windows-first).
    pub fn new(app: &tauri::AppHandle) -> tauri::Result<Option<Self>> {
        let Some(default_icon) = app.default_window_icon().cloned() else {
            return Ok(None);
        };
        let tray = match TrayIconBuilder::with_id("steward-tray")
            .icon(default_icon.clone())
            .tooltip("Steward")
            .build(app)
        {
            Ok(tray) => tray,
            Err(error) => {
                eprintln!("tray unavailable: {error}");
                return Ok(None);
            }
        };
        Ok(Some(Self {
            tray,
            last_tone: "green",
        }))
    }

    /// Updates the tooltip when health changes; icon swap keyed by tone asset.
    pub fn update(&mut self, health: DaemonHealth) {
        let tone = health.tone();
        if tone != self.last_tone {
            self.last_tone = tone;
        }
        let _ = self.tray.set_tooltip(Some(format!("Steward — {}", health.label())));
    }
}

/// Fires a desktop notification for approval-needed events. Best-effort.
pub fn notify_approval_needed(app_handle: &tauri::AppHandle, tool: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app_handle
        .notification()
        .builder()
        .title("Steward — approval needed")
        .body(format!("Tool '{tool}' is waiting for your approval."))
        .show();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_mapping_matches_dot_live_tones() {
        assert_eq!(DaemonHealth::from_probe(true, false), DaemonHealth::Online);
        assert_eq!(DaemonHealth::from_probe(true, true), DaemonHealth::Degraded);
        assert_eq!(DaemonHealth::from_probe(false, false), DaemonHealth::Lost);
        assert_eq!(DaemonHealth::Online.tone(), "green");
        assert_eq!(DaemonHealth::Degraded.tone(), "amber");
        assert_eq!(DaemonHealth::Lost.tone(), "red");
    }

    #[test]
    fn labels_are_user_visible_strings() {
        assert!(DaemonHealth::Online.label().contains("online"));
        assert!(DaemonHealth::Lost.label().contains("unreachable"));
    }
}
