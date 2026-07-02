use super::StewardShell;
use crate::client_chat;
use crate::ui::HistoryLine;
use steward_core::pb::ProviderProfileInfo;

impl StewardShell {
    pub(super) async fn refresh_provider(&mut self) {
        match client_chat::provider_profiles(&self.host).await {
            Ok(profiles) => {
                if let Some(profile) = profiles.iter().find(|profile| profile.active) {
                    self.state.active_profile = profile.profile_id.clone();
                    self.state.model = profile.model.clone();
                }
            }
            Err(error) => self.state.push_error(format!("provider status: {error}")),
        }
    }

    pub(super) async fn refresh_sessions(&mut self) {
        match client_chat::sessions(&self.host, 10).await {
            Ok(sessions) => self.state.sessions = sessions,
            Err(error) => self.state.push_error(format!("session list: {error}")),
        }
    }

    pub(super) fn start_new_session(&mut self) {
        self.state.session_id = None;
        self.state.prompt_tokens = 0;
        self.state.completion_tokens = 0;
        self.state.tool_activity.clear();
        self.state.history.clear();
        self.state.push_system("New session ready");
    }

    pub(super) async fn show_sessions(&mut self) {
        self.refresh_sessions().await;
        self.state
            .push_system(format!("{} saved sessions", self.state.sessions.len()));
        for session in &self.state.sessions {
            self.state.history.push(HistoryLine::agent(format!(
                "{} | {} | {}",
                session.session_id, session.model, session.title
            )));
        }
    }

    pub(super) async fn resume_session(&mut self, id: &str) {
        match client_chat::session(&self.host, id).await {
            Ok(session) => {
                self.state.session_id = session.summary.map(|summary| summary.session_id);
                self.state.history.clear();
                for message in session.messages {
                    let line = match message.role.as_str() {
                        "user" => HistoryLine::user(message.content),
                        "assistant" => HistoryLine::agent(message.content),
                        "tool" => HistoryLine::tool(message.content),
                        _ => HistoryLine::system(message.content),
                    };
                    self.state.history.push(line);
                }
            }
            Err(error) => self.state.push_error(error.to_string()),
        }
    }

    pub(super) async fn show_providers(&mut self) {
        match client_chat::provider_profiles(&self.host).await {
            Ok(profiles) => {
                self.state
                    .push_system(format!("{} configured providers", profiles.len()));
                for profile in profiles {
                    let marker = if profile.active { "active" } else { "ready" };
                    self.state.history.push(HistoryLine::agent(format!(
                        "{} | {} | {} | {marker}",
                        profile.profile_id, profile.provider_id, profile.model
                    )));
                }
            }
            Err(error) => self.state.push_error(error.to_string()),
        }
    }

    pub(super) async fn activate_provider(&mut self, id: &str) {
        match client_chat::activate_provider(&self.host, id).await {
            Ok(profile) => self.apply_provider(profile),
            Err(error) => self.state.push_error(error.to_string()),
        }
    }

    pub(super) async fn change_model(&mut self, model: &str) {
        let profiles = match client_chat::provider_profiles(&self.host).await {
            Ok(profiles) => profiles,
            Err(error) => return self.state.push_error(error.to_string()),
        };
        let Some(mut profile) = profiles.into_iter().find(|profile| profile.active) else {
            return self
                .state
                .push_error("Configure a provider before selecting a model");
        };
        profile.model = model.to_owned();
        match client_chat::save_provider(&self.host, profile, true).await {
            Ok(profile) => self.apply_provider(profile),
            Err(error) => self.state.push_error(error.to_string()),
        }
    }

    fn apply_provider(&mut self, profile: ProviderProfileInfo) {
        self.state.active_profile = profile.profile_id;
        self.state.model = profile.model;
        self.state.push_system("active model updated");
    }
}
