//! Shared slash-command registry for the chat palette (web) and the CLI TUI.
//!
//! Ported from can1357/oh-my-pi@a1b254047d12e143b7c6011536e918c6c35c5906
//! `packages/tui/src/fuzzy.ts` and NousResearch/hermes-agent@693641aa
//! `ui-tui/src/app/slash/fuzzyScore.ts` (both MIT). Modified for Steward:
//! tiered scoring in Rust — exact 0 / prefix 1 / substring 2, description
//! word tokens +3; lower score wins, `f64::INFINITY` = no match.

use serde::{Deserialize, Serialize};

/// One command in the shared registry. `availability` gates execution:
/// `requires_session` commands only run with an active chat session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlashCommand {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub help: String,
    #[serde(default)]
    pub requires_session: bool,
    /// Handler identifier resolved by the calling surface (web palette maps
    /// it to a client action; the TUI to a local command).
    pub handler_id: String,
}

impl SlashCommand {
    pub fn new(name: &str, help: &str, handler_id: &str) -> Self {
        Self {
            name: name.to_owned(),
            aliases: Vec::new(),
            help: help.to_owned(),
            requires_session: false,
            handler_id: handler_id.to_owned(),
        }
    }

    pub fn requires_session(mut self) -> Self {
        self.requires_session = true;
        self
    }

    pub fn alias(mut self, alias: &str) -> Self {
        self.aliases.push(alias.to_owned());
        self
    }
}

/// Lowercase the value and return it alongside its alphanumeric word tokens.
fn tokenize(value: &str) -> Vec<String> {
    let normalized = value.to_lowercase();
    let mut tokens = vec![normalized.clone()];
    tokens.extend(
        normalized
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
            .map(str::to_owned),
    );
    tokens
}

/// Trim, drop leading slashes, lowercase — `/Model ` and `model` score alike.
fn normalize_query(query: &str) -> String {
    query.trim().trim_start_matches('/').to_lowercase()
}

fn score_fields(fields: &[String], query: &str, offset: usize) -> f64 {
    for field in fields {
        if field == query || &format!("/{field}") == query {
            return offset as f64;
        }
    }
    for field in fields {
        if field.starts_with(query) || format!("/{field}").starts_with(query) {
            return offset as f64 + 1.0;
        }
    }
    for field in fields {
        if field.contains(query) {
            return offset as f64 + 2.0;
        }
    }
    f64::INFINITY
}

/// Score one command against a normalized query. Lower is better.
pub fn score(command: &SlashCommand, query: &str) -> f64 {
    let query = normalize_query(query);
    if query.is_empty() {
        return 0.0;
    }
    let mut command_fields = vec![command.name.clone()];
    command_fields.extend(command.aliases.iter().cloned());
    let command_fields: Vec<String> = command_fields.iter().flat_map(|f| tokenize(f)).collect();
    let description_fields = tokenize(&command.help);
    score_fields(&command_fields, &query, 0).min(score_fields(&description_fields, &query, 3))
}

/// Filter + stable-sort commands by score (then original order). An empty
/// query keeps the caller's order so browsing is stable.
pub fn rank<'a>(commands: &'a [SlashCommand], query: &str) -> Vec<&'a SlashCommand> {
    let normalized = normalize_query(query);
    if normalized.is_empty() {
        return commands.iter().collect();
    }
    let mut scored: Vec<(usize, f64, &SlashCommand)> = commands
        .iter()
        .enumerate()
        .map(|(index, command)| (index, score(command, &normalized), command))
        .filter(|(_, score, _)| *score != f64::INFINITY)
        .collect();
    scored.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    scored.into_iter().map(|(_, _, command)| command).collect()
}

/// The daemon-facing catalog. Client surfaces (web palette, CLI TUI) fetch
/// this via `ListSlashCommands` so both share one source of truth.
pub fn catalog() -> Vec<SlashCommand> {
    vec![
        SlashCommand::new("new", "Start a new chat session", "chat.new"),
        SlashCommand::new(
            "compact",
            "Compact this session's history into a summary",
            "chat.compact",
        )
        .requires_session(),
        SlashCommand::new("model", "Open the model picker", "chat.model"),
        SlashCommand::new("help", "List all commands", "chat.help"),
        SlashCommand::new("sessions", "Search and open a chat session", "nav.sessions"),
        SlashCommand::new("settings", "Open settings", "nav.settings"),
        SlashCommand::new(
            "refine",
            "List and apply refinement entries",
            "harness.refine",
        ),
        SlashCommand::new("agents", "Go to the agents tab", "nav.agents"),
        SlashCommand::new("workflows", "Go to the workflows tab", "nav.workflows"),
        SlashCommand::new("cron", "Go to the cron jobs tab", "nav.cron"),
        SlashCommand::new("artifacts", "Go to the artifacts tab", "nav.artifacts"),
        SlashCommand::new("tools", "Go to the tools (capabilities) tab", "nav.tools"),
    ]
}

/// Proto-shaped catalog for the `ListSlashCommands` RPC.
pub fn rpc_catalog() -> Vec<steward_core::pb::SlashCommandInfo> {
    catalog()
        .into_iter()
        .map(|command| steward_core::pb::SlashCommandInfo {
            name: command.name,
            aliases: command.aliases,
            help: command.help,
            requires_session: command.requires_session,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commands() -> Vec<SlashCommand> {
        catalog()
    }

    #[test]
    fn exact_match_beats_prefix_beats_substring() {
        let mut summary = SlashCommand::new("summary", "Summarize the session", "x");
        summary.aliases.push("summ".to_owned());
        let commands = vec![
            SlashCommand::new("unrelated", "something else entirely", "y"),
            summary,
        ];
        // exact
        assert_eq!(score(&commands[1], "summary"), 0.0);
        // alias exact (tier 0), name prefix (tier 1)
        assert_eq!(score(&commands[1], "summ"), 0.0);
        assert_eq!(score(&commands[1], "summa"), 1.0);
        // substring
        assert_eq!(score(&commands[1], "mmar"), 2.0);
        // description word (+3 tier); "summar" already hits name prefix tier 1
        // because tokenization includes the full lowercase command name.
        assert_eq!(score(&commands[1], "summarize"), 3.0);
        assert_eq!(score(&commands[1], "session"), 3.0);
        assert_eq!(score(&commands[1], "se"), 4.0);
        // no match
        assert_eq!(score(&commands[1], "zzz"), f64::INFINITY);
        assert_eq!(score(&commands[0], "summary"), f64::INFINITY);
    }

    #[test]
    fn ranking_is_stable_and_tier_ordered() {
        let commands = commands();
        let ranked = rank(&commands, "comp");
        assert_eq!(ranked[0].name, "compact");
        let ranked = rank(&commands, "history");
        // matches via description token "history" (+3 tier)
        assert!(ranked.iter().any(|command| command.name == "compact"));
    }

    #[test]
    fn leading_slash_and_case_are_normalized() {
        let commands = commands();
        assert_eq!(rank(&commands, "/COMPACT"), rank(&commands, "compact"));
        assert_eq!(rank(&commands, "").len(), commands.len());
    }

    #[test]
    fn description_match_surfaces_command_not_named_for_it() {
        // Typing a word that only appears in the description still surfaces
        // the command (hermes invariant from fuzzyScore.ts docs).
        let commands = commands();
        let ranked = rank(&commands, "jobs");
        assert!(ranked.iter().any(|command| command.name == "cron"));
    }
}
