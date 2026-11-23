//! `/find`: locate a literal in the loaded transcript and pause the viewport
//! on each hit. Each match is an entry, not an occurrence: one jump per row.

use super::{AgentView, ChatEntry, SessionUi};

/// The active `/find` search: the query, the matching entry indices (oldest
/// first), and the current match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FindState {
    query: String,
    matches: Vec<usize>,
    index: usize,
}

impl FindState {
    fn new(query: String, matches: Vec<usize>) -> Self {
        Self {
            query,
            matches,
            index: 0,
        }
    }

    fn current(&self) -> usize {
        self.matches[self.index]
    }

    fn step(&mut self, delta: isize) {
        if self.matches.is_empty() {
            return;
        }
        self.index = (self.index as isize + delta).rem_euclid(self.matches.len() as isize) as usize;
    }
}

/// The entries whose searchable text contains `query` (case-insensitive).
pub(super) fn find_matches(chat: &[ChatEntry], query: &str) -> Vec<usize> {
    let needle = query.to_lowercase();
    chat.iter()
        .enumerate()
        .filter(|(_, entry)| entry.matches_find(&needle))
        .map(|(index, _)| index)
        .collect()
}

impl SessionUi {
    /// `/find <text>`: jump to the first hit; `/find` alone clears the active
    /// search. The toast names the cycle keys and the match position.
    pub(super) fn handle_find(&mut self, query: &str, view: &mut AgentView) {
        let query = query.trim();
        if query.is_empty() {
            if self.find.take().is_some() {
                self.toast("find cleared", view);
            } else {
                view.editor.set_text("/find ");
                self.error_row("Usage: /find <text>", view);
            }
            self.dirty = true;
            return;
        }
        let matches = find_matches(&view.chat, query);
        if matches.is_empty() {
            self.find = None;
            self.toast(&format!("find: no matches for \"{query}\""), view);
            self.dirty = true;
            return;
        }
        self.find = Some(FindState::new(query.to_string(), matches));
        self.show_find(view);
    }

    /// Move to the next (`1`) or previous (`-1`) match, wrapping at either
    /// end.
    pub(super) fn step_find(&mut self, delta: isize, view: &mut AgentView) {
        let Some(find) = self.find.as_mut() else {
            return;
        };
        find.step(delta);
        self.show_find(view);
    }

    fn show_find(&mut self, view: &mut AgentView) {
        let Some(find) = self.find.as_ref() else {
            return;
        };
        let (query, index, position, total) = (
            find.query.clone(),
            find.current(),
            find.index + 1,
            find.matches.len(),
        );
        if index >= view.chat.len() {
            // A rebuild that kept a stale state (a defensive arm: the
            // transcript replacements clear it) must not index past the chat.
            self.find = None;
            return;
        }
        view.scroll_to_entry(index, 0);
        let (next, previous) = {
            let kb = view.editor.keybindings();
            let display = |id: &str| {
                kb.first_key(id)
                    .map(|key| crate::keybindings::format_key_text(&key))
                    .unwrap_or_default()
            };
            (
                display("tui.viewport.findNext"),
                display("tui.viewport.findPrevious"),
            )
        };
        self.toast(
            &format!("find \"{query}\" {position}/{total} · {next} / {previous} · /find clears"),
            view,
        );
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{AssistantMessage, MessageBlock, StatusKind};

    fn assistant(text: &str) -> ChatEntry {
        ChatEntry::Assistant(Box::new(AssistantMessage {
            blocks: vec![MessageBlock::Text(text.to_string())],
            has_tool_calls: false,
            streaming: false,
            error: None,
            aborted: false,
        }))
    }

    #[test]
    fn find_matches_are_case_insensitive_and_skip_hidden_thinking() {
        let chat = vec![
            ChatEntry::User {
                text: "Fix the Parser".to_string(),
            },
            assistant("the parser is fixed"),
            assistant("parser notes"),
        ];
        assert_eq!(find_matches(&chat, "PARSER"), vec![0, 1, 2]);
        assert_eq!(find_matches(&chat, "missing"), Vec::<usize>::new());

        let thinking = ChatEntry::Assistant(Box::new(AssistantMessage {
            blocks: vec![MessageBlock::Thinking("parser".to_string())],
            has_tool_calls: false,
            streaming: false,
            error: None,
            aborted: false,
        }));
        assert_eq!(find_matches(&[thinking], "parser"), Vec::<usize>::new());
    }

    #[test]
    fn find_matches_user_slash_status_and_compaction_rows() {
        let chat = vec![
            ChatEntry::SlashCommand {
                text: "/model gpt".to_string(),
            },
            ChatEntry::Status {
                text: "compaction finished".to_string(),
                kind: StatusKind::Info,
            },
            ChatEntry::CompactionSummary {
                summary: "the summary".to_string(),
                tokens_before: 0,
                custom_instructions: Some("focus".to_string()),
            },
        ];
        assert_eq!(find_matches(&chat, "gpt"), vec![0]);
        assert_eq!(find_matches(&chat, "finished"), vec![1]);
        assert_eq!(find_matches(&chat, "focus"), vec![2]);
    }

    #[test]
    fn stepping_wraps_both_ways() {
        let mut find = FindState::new("x".to_string(), vec![3, 7, 9]);
        assert_eq!(find.current(), 3);
        find.step(1);
        assert_eq!(find.current(), 7);
        find.step(1);
        assert_eq!(find.current(), 9);
        find.step(1);
        assert_eq!(find.current(), 3);
        find.step(-1);
        assert_eq!(find.current(), 9);
    }
}
