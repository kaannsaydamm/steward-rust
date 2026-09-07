//! Ported from NousResearch/hermes-agent@693641aa8b4359c602283bdbbc14041e03bc47bc
//! desktop shell menu architecture (MIT). Modified for Steward: native
//! Tauri v2 menu (File/Session/View) emitting typed events the WebUI can
//! consume; no Electron.

use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder};

pub const EVT_NEW_SESSION: &str = "steward://menu/new-session";
pub const EVT_OPEN_SETTINGS: &str = "steward://menu/open-settings";
pub const EVT_CYCLE_FONT: &str = "steward://menu/cycle-font";
pub const EVT_CYCLE_WIDTH: &str = "steward://menu/cycle-width";

/// Builds the application menu. Menu items emit the event ids above through
pub fn build_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let new_session = MenuItemBuilder::with_id("new-session", "New Session").build(app)?;
    let open_settings = MenuItemBuilder::with_id("open-settings", "Open Settings…").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Steward").build(app)?;
    let cycle_font = MenuItemBuilder::with_id("cycle-font", "Cycle Transcript Size").build(app)?;
    let cycle_width = MenuItemBuilder::with_id("cycle-width", "Cycle Transcript Width").build(app)?;

    let file_submenu = tauri::menu::SubmenuBuilder::new(app, "File")
        .item(&new_session)
        .item(&open_settings)
        .separator()
        .item(&quit)
        .build()?;
    let session_submenu = tauri::menu::SubmenuBuilder::new(app, "Session")
        .item(&new_session)
        .build()?;
    let view_submenu = tauri::menu::SubmenuBuilder::new(app, "View")
        .item(&cycle_font)
        .item(&cycle_width)
        .build()?;

    MenuBuilder::new(app)
        .item(&file_submenu)
        .item(&session_submenu)
        .item(&view_submenu)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_ids_are_stable_contracts() {
        // The WebUI listens for these exact ids; renaming breaks the shell.
        assert_eq!(EVT_NEW_SESSION, "steward://menu/new-session");
        assert_eq!(EVT_OPEN_SETTINGS, "steward://menu/open-settings");
        assert_eq!(EVT_CYCLE_FONT, "steward://menu/cycle-font");
        assert_eq!(EVT_CYCLE_WIDTH, "steward://menu/cycle-width");
    }
}
