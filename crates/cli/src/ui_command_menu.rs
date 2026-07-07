use super::{ShellState, BORDER, GOLD, MUTED, PARCHMENT};
use crate::interactive_help;
use ratatui::layout::Rect;
use ratatui::prelude::{Frame, Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem};

const MAX_VISIBLE: usize = 8;

/// The slash-command filter implied by the current input, if the live autocomplete menu
/// should be showing: input starts with '/' and the command name isn't finished yet (no
/// space typed after it).
pub fn active_filter(input: &str) -> Option<&str> {
    let rest = input.strip_prefix('/')?;
    if rest.contains(char::is_whitespace) {
        return None;
    }
    Some(rest)
}

/// The top autocomplete match for `filter`, used by Tab-completion.
pub fn best_match(filter: &str) -> Option<&'static str> {
    interactive_help::matches(filter)
        .first()
        .map(|(name, _)| *name)
}

/// Draws the live autocomplete popup directly above `composer_area`, overlapping the
/// bottom of the transcript region. Does nothing if the input doesn't imply a menu or no
/// command matches.
pub fn draw(frame: &mut Frame<'_>, transcript_area: Rect, composer_area: Rect, state: &ShellState) {
    let Some(filter) = active_filter(&state.input) else {
        return;
    };
    let matches = interactive_help::matches(filter);
    if matches.is_empty() {
        return;
    }
    let visible = matches.len().min(MAX_VISIBLE);
    let height = (visible as u16) + 2;
    let y = composer_area
        .y
        .saturating_sub(height)
        .max(transcript_area.y);
    let area = Rect {
        x: transcript_area.x,
        y,
        width: transcript_area.width,
        height: height.min(composer_area.y.saturating_sub(transcript_area.y)),
    };
    if area.height < 3 {
        return;
    }

    let items: Vec<ListItem> = matches
        .iter()
        .take(visible)
        .enumerate()
        .map(|(index, (name, description))| {
            let style = if index == 0 {
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(PARCHMENT)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{name:<18}"), style),
                Span::styled(*description, Style::default().fg(MUTED)),
            ]))
        })
        .collect();

    frame.render_widget(Clear, area);
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title("commands (Tab to complete)")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER)),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::{active_filter, best_match};

    #[test]
    fn active_filter_extracts_the_partial_command_name() {
        assert_eq!(active_filter("/cro"), Some("cro"));
        assert_eq!(active_filter("/"), Some(""));
    }

    #[test]
    fn active_filter_is_none_once_a_space_is_typed() {
        assert_eq!(active_filter("/cron list"), None);
    }

    #[test]
    fn active_filter_is_none_for_plain_text() {
        assert_eq!(active_filter("hello"), None);
    }

    #[test]
    fn best_match_picks_the_top_ranked_command() {
        assert_eq!(best_match("cron"), Some("/cron"));
    }

    #[test]
    fn best_match_is_none_when_nothing_matches() {
        assert_eq!(best_match("zzz-not-a-command"), None);
    }
}
