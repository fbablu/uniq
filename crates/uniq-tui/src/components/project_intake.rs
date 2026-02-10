//! Phase 1: Project Intake — user provides project path and description.
//!
//! Features:
//! - Path field: single-line with filesystem autocomplete
//! - Description field: multi-line text area with scroll viewport
//! - Tab to accept path suggestions or switch fields
//! - Enter inserts newlines in description, navigates in path
//! - Ctrl+Enter submits the form for analysis

use std::path::{Path, PathBuf};

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::action::Action;
use crate::components::Component;
use crate::theme::Theme;

use uniq_core::project::ProjectProfile;

/// Maximum number of path suggestions to display.
const MAX_SUGGESTIONS: usize = 8;

/// Which input field is currently focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputField {
    Path,
    Description,
}

pub struct ProjectIntakeComponent {
    /// Current project path input.
    pub path_input: String,
    /// Current description input (may contain newlines).
    pub description_input: String,
    /// Which field is focused.
    focused: InputField,
    /// Cursor position (byte offset) within the focused field.
    pub cursor: usize,
    /// Whether the project has been analyzed.
    pub profile: Option<ProjectProfile>,
    /// Whether analysis is in progress.
    pub analyzing: bool,
    /// Error message if analysis failed.
    pub error: Option<String>,

    // ── Description viewport ─────────────────────────────────
    /// Scroll offset (line number of the first visible line in the description).
    desc_scroll: usize,

    // ── Path suggestions ────────────────────────────────────
    /// Current filesystem suggestions based on path_input.
    suggestions: Vec<PathSuggestion>,
    /// Which suggestion is highlighted (-1 = none).
    suggestion_index: Option<usize>,
    /// The path input value that was last used to compute suggestions
    /// (avoids recomputing on every render).
    suggestions_for: String,
}

/// A single path suggestion entry.
#[derive(Debug, Clone)]
struct PathSuggestion {
    /// The full absolute path.
    full_path: String,
    /// Just the filename/dirname component (for display).
    name: String,
    /// Whether this is a directory.
    is_dir: bool,
}

impl ProjectIntakeComponent {
    pub fn new() -> Self {
        let mut this = Self {
            path_input: "~/".to_string(),
            description_input: String::new(),
            focused: InputField::Path,
            cursor: 2,
            profile: None,
            analyzing: false,
            error: None,
            desc_scroll: 0,
            suggestions: Vec::new(),
            suggestion_index: None,
            suggestions_for: String::new(),
        };
        this.refresh_suggestions();
        this
    }

    /// Whether this component wants to capture raw key input.
    pub fn wants_input(&self) -> bool {
        self.profile.is_none() && !self.analyzing
    }

    /// Get a reference to the currently focused input string.
    fn focused_input(&self) -> &str {
        match self.focused {
            InputField::Path => &self.path_input,
            InputField::Description => &self.description_input,
        }
    }

    /// Clamp cursor to valid range for the focused field.
    fn clamp_cursor(&mut self) {
        let len = self.focused_input().len();
        if self.cursor > len {
            self.cursor = len;
        }
    }

    /// Insert a character at the cursor position.
    fn insert_char(&mut self, c: char) {
        self.clamp_cursor();
        let cursor = self.cursor;
        let input = match self.focused {
            InputField::Path => &mut self.path_input,
            InputField::Description => &mut self.description_input,
        };
        input.insert(cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Delete the character before the cursor.
    fn delete_char(&mut self) {
        self.clamp_cursor();
        if self.cursor > 0 {
            let cursor = self.cursor;
            let input = match self.focused {
                InputField::Path => &mut self.path_input,
                InputField::Description => &mut self.description_input,
            };
            let prev = input[..cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            input.remove(prev);
            self.cursor = prev;
        }
    }

    /// Delete the word before the cursor (Ctrl+W).
    fn delete_word(&mut self) {
        self.clamp_cursor();
        if self.cursor > 0 {
            let cursor = self.cursor;
            let input = match self.focused {
                InputField::Path => &mut self.path_input,
                InputField::Description => &mut self.description_input,
            };
            let mut end = cursor;
            while end > 0 && input.as_bytes().get(end - 1) == Some(&b' ') {
                end -= 1;
            }
            let mut start = end;
            while start > 0 && input.as_bytes().get(start - 1) != Some(&b' ') {
                start -= 1;
            }
            input.drain(start..cursor);
            self.cursor = start;
        }
    }

    /// Insert a newline at the cursor position (description field only).
    fn insert_newline(&mut self) {
        if self.focused == InputField::Description {
            self.insert_char('\n');
            self.ensure_cursor_visible();
        }
    }

    /// Insert a string at the cursor position (for paste).
    fn insert_str(&mut self, s: &str) {
        self.clamp_cursor();
        let cursor = self.cursor;
        let input = match self.focused {
            InputField::Path => &mut self.path_input,
            InputField::Description => &mut self.description_input,
        };
        input.insert_str(cursor, s);
        self.cursor += s.len();
        if self.focused == InputField::Description {
            self.ensure_cursor_visible();
        }
    }

    /// Get the line number and column of the cursor within the description.
    fn cursor_line_col(&self, text: &str, cursor: usize) -> (usize, usize) {
        let before = &text[..cursor.min(text.len())];
        let line = before.matches('\n').count();
        let col = before.rfind('\n').map(|p| cursor - p - 1).unwrap_or(cursor);
        (line, col)
    }

    /// Move cursor up one line in the description.
    fn cursor_up(&mut self) {
        if self.focused != InputField::Description {
            return;
        }
        let text = &self.description_input;
        let (line, col) = self.cursor_line_col(text, self.cursor);
        if line == 0 {
            return; // Already on first line.
        }
        // Find the start of the previous line.
        let lines: Vec<&str> = text.split('\n').collect();
        let prev_line = lines[line - 1];
        let prev_line_start: usize = lines[..line - 1].iter().map(|l| l.len() + 1).sum();
        self.cursor = prev_line_start + col.min(prev_line.len());
        self.ensure_cursor_visible();
    }

    /// Move cursor down one line in the description.
    fn cursor_down(&mut self) {
        if self.focused != InputField::Description {
            return;
        }
        let text = &self.description_input;
        let lines: Vec<&str> = text.split('\n').collect();
        let (line, col) = self.cursor_line_col(text, self.cursor);
        if line + 1 >= lines.len() {
            return; // Already on last line.
        }
        let next_line = lines[line + 1];
        let next_line_start: usize = lines[..line + 1].iter().map(|l| l.len() + 1).sum();
        self.cursor = next_line_start + col.min(next_line.len());
        self.ensure_cursor_visible();
    }

    /// Ensure the cursor's line is visible within the scroll viewport.
    /// Uses a conservative viewport estimate (actual height adjusted at render).
    fn ensure_cursor_visible(&mut self) {
        let (cursor_line, _) = self.cursor_line_col(&self.description_input, self.cursor);
        if cursor_line < self.desc_scroll {
            self.desc_scroll = cursor_line;
        }
        // For scrolling down, we use a conservative estimate.
        // Render will further adjust if needed, but we do our best here.
        // Assume at least 6 lines of viewport (minimum height minus border).
        let estimated_viewport = 6usize;
        if cursor_line >= self.desc_scroll + estimated_viewport {
            self.desc_scroll = cursor_line.saturating_sub(estimated_viewport - 1);
        }
    }

    /// Switch focus to the other input field.
    fn switch_field(&mut self) {
        self.focused = match self.focused {
            InputField::Path => InputField::Description,
            InputField::Description => InputField::Path,
        };
        self.cursor = self.focused_input().len();
        // Reset suggestions when leaving path field.
        if self.focused != InputField::Path {
            self.suggestions.clear();
            self.suggestion_index = None;
        }
    }

    // ── Path suggestion logic ───────────────────────────────

    /// Refresh filesystem suggestions based on the current path_input.
    /// Called after every keystroke when the path field is focused.
    fn refresh_suggestions(&mut self) {
        // Only compute if the input actually changed.
        if self.path_input == self.suggestions_for {
            return;
        }
        self.suggestions_for = self.path_input.clone();
        self.suggestion_index = None;
        self.suggestions.clear();

        let input = &self.path_input;
        if input.is_empty() {
            return;
        }

        // Expand ~ to home directory.
        let expanded = if input.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                home.to_string_lossy().to_string() + &input[1..]
            } else {
                input.clone()
            }
        } else {
            input.clone()
        };

        let path = Path::new(&expanded);

        // Determine the parent directory to list, and the prefix to filter by.
        let (search_dir, prefix): (PathBuf, String) =
            if expanded.ends_with('/') || expanded.ends_with(std::path::MAIN_SEPARATOR) {
                // User typed a trailing slash — list contents of this directory.
                (path.to_path_buf(), String::new())
            } else if path.is_dir() && !input.contains('.') {
                // The current input IS a complete directory — list its contents.
                // (But only if it doesn't look like the user is mid-filename.)
                (path.to_path_buf(), String::new())
            } else {
                // Partial name — list parent, filter by filename prefix.
                let parent = path.parent().unwrap_or(Path::new("/"));
                let file_prefix = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                (parent.to_path_buf(), file_prefix)
            };

        // Read the directory entries.
        let entries = match std::fs::read_dir(&search_dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        let prefix_lower = prefix.to_lowercase();

        let mut results: Vec<PathSuggestion> = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip hidden files unless the user is explicitly typing a dot.
                if name.starts_with('.') && !prefix.starts_with('.') {
                    return None;
                }

                // Filter by prefix (case-insensitive).
                if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix_lower) {
                    return None;
                }

                let full_path = entry.path();
                let is_dir = full_path.is_dir();

                Some(PathSuggestion {
                    full_path: full_path.to_string_lossy().to_string(),
                    name,
                    is_dir,
                })
            })
            .collect();

        // Sort: directories first, then alphabetically.
        results.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        // Truncate to max.
        results.truncate(MAX_SUGGESTIONS);

        self.suggestions = results;
    }

    /// Accept the currently highlighted suggestion (or the first one).
    fn accept_suggestion(&mut self) {
        let idx = self.suggestion_index.unwrap_or(0);
        if let Some(suggestion) = self.suggestions.get(idx) {
            let mut new_path = suggestion.full_path.clone();
            // If it's a directory, append a slash so the user can keep drilling.
            if suggestion.is_dir && !new_path.ends_with('/') {
                new_path.push('/');
            }
            self.path_input = new_path;
            self.cursor = self.path_input.len();
            // Clear old suggestions so they refresh on next input.
            self.suggestions_for.clear();
            self.suggestions.clear();
            self.suggestion_index = None;
            // Immediately refresh for the new path.
            self.refresh_suggestions();
        }
    }

    /// Try to submit the form. Returns the action to dispatch, or a status message.
    fn try_submit(&mut self) -> Option<Action> {
        // Dismiss any suggestions.
        self.suggestions.clear();
        self.suggestion_index = None;

        if !self.path_input.is_empty() && !self.description_input.is_empty() {
            // Expand ~ before submitting.
            let path = if self.path_input.starts_with('~') {
                if let Some(home) = dirs::home_dir() {
                    home.to_string_lossy().to_string() + &self.path_input[1..]
                } else {
                    self.path_input.clone()
                }
            } else {
                self.path_input.clone()
            };

            self.analyzing = true;
            self.error = None;
            Some(Action::SubmitProject {
                path,
                description: self.description_input.clone(),
            })
        } else if self.path_input.is_empty() {
            Some(Action::SetStatus("Enter a project path first".to_string()))
        } else {
            Some(Action::SetStatus(
                "Enter a description of what AI capability to add".to_string(),
            ))
        }
    }

    /// Whether suggestions are currently visible.
    fn has_suggestions(&self) -> bool {
        self.focused == InputField::Path && !self.suggestions.is_empty()
    }

    /// Render the path input field with cursor.
    fn render_text_field(
        text: &str,
        cursor: usize,
        is_focused: bool,
        placeholder: &str,
        title: &str,
        frame: &mut Frame,
        area: Rect,
    ) {
        let border_style = if is_focused {
            Style::default().fg(Theme::accent())
        } else {
            Theme::border()
        };
        let block = Block::default()
            .title(title)
            .title_style(if is_focused {
                Theme::key_hint()
            } else {
                Theme::muted()
            })
            .borders(Borders::ALL)
            .border_style(border_style);

        let display = if text.is_empty() && !is_focused {
            Paragraph::new(Span::styled(placeholder, Theme::dim()))
        } else if is_focused {
            let pos = cursor.min(text.len());
            let (before, after) = text.split_at(pos);
            let cursor_char = if after.is_empty() {
                " ".to_string()
            } else {
                after.chars().next().unwrap().to_string()
            };
            let rest = if after.len() > cursor_char.len() {
                &after[cursor_char.len()..]
            } else {
                ""
            };
            Paragraph::new(Line::from(vec![
                Span::styled(before, Theme::normal()),
                Span::styled(
                    cursor_char,
                    Style::default().fg(Theme::bg()).bg(Theme::accent()),
                ),
                Span::styled(rest, Theme::normal()),
            ]))
        } else {
            Paragraph::new(Span::styled(text, Theme::normal()))
        };

        frame.render_widget(display.block(block), area);
    }
}

impl Component for ProjectIntakeComponent {
    fn handle_action(&mut self, action: &Action) -> Option<Action> {
        match action {
            // ── Text input ──────────────────────────────────────
            Action::CharInput(c) => {
                self.insert_char(*c);
                if self.focused == InputField::Path {
                    self.refresh_suggestions();
                }
                None
            }
            Action::BackspaceInput => {
                self.delete_char();
                if self.focused == InputField::Path {
                    self.refresh_suggestions();
                }
                if self.focused == InputField::Description {
                    self.ensure_cursor_visible();
                }
                None
            }
            Action::DeleteWord => {
                self.delete_word();
                if self.focused == InputField::Path {
                    self.refresh_suggestions();
                }
                if self.focused == InputField::Description {
                    self.ensure_cursor_visible();
                }
                None
            }
            Action::PasteInput => {
                // Try to read from clipboard via pbpaste (macOS fallback).
                if let Ok(output) = std::process::Command::new("pbpaste").output() {
                    if let Ok(text) = String::from_utf8(output.stdout) {
                        if !text.is_empty() {
                            let to_paste = if self.focused == InputField::Path {
                                text.lines().next().unwrap_or("").to_string()
                            } else {
                                text
                            };
                            self.insert_str(&to_paste);
                            if self.focused == InputField::Path {
                                self.refresh_suggestions();
                            }
                        }
                    }
                }
                None
            }
            Action::PasteBulk(text) => {
                // Bracketed paste — terminal sent the entire pasted text at once.
                if !text.is_empty() {
                    let to_paste = if self.focused == InputField::Path {
                        // Path field: only first line, no newlines.
                        text.lines().next().unwrap_or("").to_string()
                    } else {
                        text.clone()
                    };
                    self.insert_str(&to_paste);
                    if self.focused == InputField::Path {
                        self.refresh_suggestions();
                    }
                }
                None
            }

            // ── Tab: accept suggestion OR switch field ──────────
            Action::SwitchInputField => {
                if self.has_suggestions() {
                    // Accept the highlighted suggestion.
                    self.accept_suggestion();
                } else {
                    self.switch_field();
                }
                None
            }

            // ── Up/Down ─────────────────────────────────────────
            Action::ScrollDown | Action::SelectNext => {
                if self.has_suggestions() {
                    let max = self.suggestions.len();
                    self.suggestion_index = Some(match self.suggestion_index {
                        None => 0,
                        Some(i) => (i + 1).min(max - 1),
                    });
                } else if self.focused == InputField::Description {
                    // Move cursor down a line in multi-line description.
                    self.cursor_down();
                } else if self.focused == InputField::Path {
                    self.switch_field();
                }
                None
            }
            Action::ScrollUp | Action::SelectPrev => {
                if self.has_suggestions() {
                    self.suggestion_index = match self.suggestion_index {
                        None | Some(0) => None,
                        Some(i) => Some(i - 1),
                    };
                } else if self.focused == InputField::Description {
                    let (line, _) = self.cursor_line_col(&self.description_input, self.cursor);
                    if line == 0 {
                        // On first line of description — switch to path field.
                        self.switch_field();
                    } else {
                        self.cursor_up();
                    }
                }
                None
            }

            // ── Enter: newline in description, navigate in path ─
            Action::NewlineInput => {
                match self.focused {
                    InputField::Path => {
                        // In path field, Enter behaves like before:
                        // accept navigated suggestion, or move to description.
                        if self.has_suggestions() && self.suggestion_index.is_some() {
                            self.accept_suggestion();
                            return None;
                        }
                        // Dismiss un-navigated suggestions.
                        self.suggestions.clear();
                        self.suggestion_index = None;
                        // Move focus to description.
                        if !self.path_input.is_empty() {
                            self.switch_field();
                        }
                    }
                    InputField::Description => {
                        // Insert a newline character.
                        self.insert_newline();
                    }
                }
                None
            }

            // ── Ctrl+Enter: submit the form ─────────────────────
            Action::SubmitForm => self.try_submit(),

            // ── Legacy Confirm (from normal mode Enter) ─────────
            Action::Confirm => self.try_submit(),

            // ── Async results ───────────────────────────────────
            Action::ProjectAnalyzed(profile) => {
                self.analyzing = false;
                self.profile = Some(*profile.clone());
                None
            }
            Action::ProjectAnalysisFailed(err) => {
                self.analyzing = false;
                self.error = Some(err.clone());
                None
            }
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        // No outer block — content fills the area directly.
        let inner = area;

        // Calculate suggestion area height.
        let suggestion_height = if self.has_suggestions() {
            (self.suggestions.len() as u16 + 2).min(MAX_SUGGESTIONS as u16 + 2) // +2 for border
        } else {
            0
        };

        // When profile is available, show it in the bottom area.
        // When not yet submitted, give description the bulk of the space.
        let status_height = if self.profile.is_some() || self.analyzing || self.error.is_some() {
            8u16
        } else if !self.wants_input() {
            6
        } else {
            0 // No status area — give all space to description
        };

        let chunks = Layout::vertical([
            Constraint::Length(3),                 // Path input
            Constraint::Length(suggestion_height), // Suggestions dropdown
            Constraint::Min(8), // Description input (multi-line, takes remaining)
            Constraint::Length(2), // Instructions
            Constraint::Length(status_height), // Profile display or status
        ])
        .split(inner);

        // ── Path input field ────────────────────────────────────
        let path_focused = self.focused == InputField::Path && self.wants_input();
        Self::render_text_field(
            &self.path_input,
            self.cursor,
            path_focused,
            "/path/to/your/project",
            " Project Path ",
            frame,
            chunks[0],
        );

        // ── Suggestions dropdown ────────────────────────────────
        if self.has_suggestions() {
            let suggestion_block = Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .border_style(Theme::border());

            let items: Vec<ListItem> = self
                .suggestions
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let is_highlighted = self.suggestion_index == Some(i);

                    let icon = if s.is_dir { "/" } else { " " };

                    let style = if is_highlighted {
                        Style::default()
                            .fg(Theme::bg())
                            .bg(Theme::accent())
                            .add_modifier(Modifier::BOLD)
                    } else {
                        #[allow(clippy::collapsible_else_if)]
                        if s.is_dir {
                            Style::default().fg(Theme::accent())
                        } else {
                            Theme::normal()
                        }
                    };

                    ListItem::new(Line::from(vec![Span::styled(
                        format!(" {}{} ", s.name, icon),
                        style,
                    )]))
                })
                .collect();

            let list = List::new(items).block(suggestion_block);
            frame.render_widget(list, chunks[1]);
        }

        // ── Description input field (multi-line with scroll) ─────
        let desc_focused = self.focused == InputField::Description && self.wants_input();
        self.render_description_field(desc_focused, frame, chunks[2]);

        // ── Instructions ────────────────────────────────────────
        let instructions = if self.has_suggestions() {
            Paragraph::new(Line::from(vec![
                Span::styled("  tab", Theme::key_hint()),
                Span::styled(" accept  ", Theme::dim()),
                Span::styled("↑↓", Theme::key_hint()),
                Span::styled(" navigate  ", Theme::dim()),
                Span::styled("enter", Theme::key_hint()),
                Span::styled(" next field", Theme::dim()),
            ]))
        } else if self.wants_input() && self.focused == InputField::Description {
            Paragraph::new(Line::from(vec![
                Span::styled("  ctrl+s", Theme::key_hint()),
                Span::styled(" submit  ", Theme::dim()),
                Span::styled("tab", Theme::key_hint()),
                Span::styled(" switch  ", Theme::dim()),
                Span::styled("ctrl+v", Theme::key_hint()),
                Span::styled(" paste", Theme::dim()),
            ]))
        } else if self.wants_input() {
            Paragraph::new(Line::from(vec![
                Span::styled("  enter", Theme::key_hint()),
                Span::styled(" next field  ", Theme::dim()),
                Span::styled("ctrl+s", Theme::key_hint()),
                Span::styled(" submit  ", Theme::dim()),
                Span::styled("tab", Theme::key_hint()),
                Span::styled(" switch", Theme::dim()),
            ]))
        } else {
            Paragraph::new(Line::from(vec![
                Span::styled("  →", Theme::key_hint()),
                Span::styled(" next phase", Theme::dim()),
            ]))
        };
        frame.render_widget(instructions, chunks[3]);

        // ── Status / Profile display ────────────────────────────
        if status_height == 0 && !self.analyzing && self.error.is_none() && self.profile.is_none() {
            // No status to show — description fills the space.
        } else if self.analyzing {
            let spinner = Paragraph::new(Span::styled(
                "Analyzing project...",
                Style::default().fg(Theme::warning()),
            ));
            frame.render_widget(spinner, chunks[4]);
        } else if let Some(ref error) = self.error {
            let err = Paragraph::new(Span::styled(
                format!("Error: {}", error),
                Style::default().fg(Theme::error()),
            ))
            .wrap(Wrap { trim: true });
            frame.render_widget(err, chunks[4]);
        } else if let Some(ref profile) = self.profile {
            let lines = vec![
                Line::from(vec![
                    Span::styled("Languages: ", Theme::header()),
                    Span::styled(format!("{:?}", profile.languages), Theme::normal()),
                ]),
                Line::from(vec![
                    Span::styled("Files: ", Theme::header()),
                    Span::styled(format!("{}", profile.file_count), Theme::normal()),
                ]),
                Line::from(vec![
                    Span::styled("Summary: ", Theme::header()),
                    Span::styled(&profile.summary, Theme::normal()),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Press [Right] to proceed to Research Discovery",
                    Theme::selected(),
                )),
            ];
            let profile_display = Paragraph::new(lines).wrap(Wrap { trim: true });
            frame.render_widget(profile_display, chunks[4]);
        }
    }
}

impl ProjectIntakeComponent {
    /// Render the multi-line description text area with scrolling viewport.
    fn render_description_field(&self, is_focused: bool, frame: &mut Frame, area: Rect) {
        let border_style = if is_focused {
            Style::default().fg(Theme::accent())
        } else {
            Theme::border()
        };

        let text = &self.description_input;
        let line_count = text.split('\n').count();

        // Title shows line count when content is long.
        let title = if text.is_empty() {
            " Describe what AI capability to add (multi-line) ".to_string()
        } else {
            format!(
                " Description ({} line{}, {} chars) ",
                line_count,
                if line_count == 1 { "" } else { "s" },
                text.len()
            )
        };

        let block = Block::default()
            .title(title.clone())
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(area);
        let viewport_height = inner_area.height as usize;

        // Placeholder when empty and not focused.
        if text.is_empty() && !is_focused {
            let placeholder = Paragraph::new(Span::styled(
                "Paste or type a detailed description of the AI capability you want.\n\
                 The more detail you provide (research concepts, techniques, papers),\n\
                 the better uniq can search for relevant approaches.",
                Theme::dim(),
            ))
            .wrap(Wrap { trim: true })
            .block(block);
            frame.render_widget(placeholder, area);
            return;
        }

        if !is_focused {
            // Not focused — just show the text with wrapping.
            let display = Paragraph::new(text.as_str())
                .style(Theme::normal())
                .wrap(Wrap { trim: false })
                .scroll((self.desc_scroll as u16, 0))
                .block(block);
            frame.render_widget(display, area);
            return;
        }

        // ── Focused: render with cursor, word-wrap, and scroll ─────
        let viewport_width = inner_area.width as usize;
        let wrap_width = if viewport_width > 0 {
            viewport_width
        } else {
            80
        };

        // Word-wrap each logical line into visual lines.
        // Track: (visual_line_text, is_cursor_on_this_line, cursor_col_in_visual_line)
        struct VisualLine {
            text: String,
            cursor_col: Option<usize>, // Some(col) if cursor is on this visual line
        }

        let logical_lines: Vec<&str> = text.split('\n').collect();
        let (cursor_logical, cursor_col_in_logical) = self.cursor_line_col(text, self.cursor);

        let mut visual_lines: Vec<VisualLine> = Vec::new();
        let mut cursor_visual_line: usize = 0;

        for (li, logical_text) in logical_lines.iter().enumerate() {
            let is_cursor_logical = li == cursor_logical;
            let wrapped = wrap_line(logical_text, wrap_width);

            if wrapped.is_empty() {
                // Empty line.
                let vl = VisualLine {
                    text: String::new(),
                    cursor_col: if is_cursor_logical { Some(0) } else { None },
                };
                if is_cursor_logical {
                    cursor_visual_line = visual_lines.len();
                }
                visual_lines.push(vl);
            } else {
                let mut col_offset = 0usize;
                for segment in &wrapped {
                    let seg_len = segment.len();
                    let cursor_col = if is_cursor_logical {
                        let c = cursor_col_in_logical;
                        if c >= col_offset && c <= col_offset + seg_len {
                            // Cursor is on this visual line, unless it's at the
                            // exact boundary and there's a next segment.
                            if c == col_offset + seg_len
                                && col_offset + seg_len < logical_text.len()
                            {
                                None // Will be on the next visual line.
                            } else {
                                cursor_visual_line = visual_lines.len();
                                Some(c - col_offset)
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    visual_lines.push(VisualLine {
                        text: segment.clone(),
                        cursor_col,
                    });
                    col_offset += seg_len;
                }
                // Edge case: cursor at very end of logical line, past last segment.
                if is_cursor_logical
                    && visual_lines
                        .last()
                        .map(|v| v.cursor_col.is_none())
                        .unwrap_or(true)
                {
                    // Cursor hasn't been placed yet — put it at end of last visual line.
                    if let Some(last) = visual_lines.last_mut() {
                        last.cursor_col = Some(last.text.len());
                        cursor_visual_line = visual_lines.len() - 1;
                    }
                }
            }
        }

        let total_visual = visual_lines.len();

        // Adjust scroll based on visual lines.
        let scroll = {
            let mut s = self.desc_scroll;
            if cursor_visual_line < s {
                s = cursor_visual_line;
            }
            if viewport_height > 0 && cursor_visual_line >= s + viewport_height {
                s = cursor_visual_line - viewport_height + 1;
            }
            s
        };

        // Build rendered lines for the visible viewport.
        let mut rendered_lines: Vec<Line> = Vec::new();
        for i in scroll..total_visual {
            if rendered_lines.len() >= viewport_height {
                break;
            }
            let vl = &visual_lines[i];
            if let Some(col) = vl.cursor_col {
                let col = col.min(vl.text.len());
                let (before, after) = vl.text.split_at(col);
                let cursor_char = if after.is_empty() {
                    " ".to_string()
                } else {
                    after.chars().next().unwrap().to_string()
                };
                let rest = if after.len() > cursor_char.len() {
                    &after[cursor_char.len()..]
                } else {
                    ""
                };
                rendered_lines.push(Line::from(vec![
                    Span::styled(before, Theme::normal()),
                    Span::styled(
                        cursor_char,
                        Style::default().fg(Theme::bg()).bg(Theme::accent()),
                    ),
                    Span::styled(rest, Theme::normal()),
                ]));
            } else {
                rendered_lines.push(Line::from(Span::styled(&vl.text, Theme::normal())));
            }
        }

        // Show scroll indicator in border if content overflows.
        let has_more_above = scroll > 0;
        let has_more_below = scroll + viewport_height < total_visual;
        let scroll_hint = if has_more_above && has_more_below {
            format!(
                " [{}/{} vis.lines] ",
                scroll + viewport_height,
                total_visual
            )
        } else if has_more_below {
            format!(" [{} more below] ", total_visual - scroll - viewport_height)
        } else if has_more_above {
            format!(" [{} above] ", scroll)
        } else {
            String::new()
        };

        let block = if !scroll_hint.is_empty() {
            Block::default()
                .title(title)
                .title_bottom(Line::from(Span::styled(&scroll_hint, Theme::dim())))
                .borders(Borders::ALL)
                .border_style(border_style)
        } else {
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style)
        };

        let display = Paragraph::new(rendered_lines).block(block);
        frame.render_widget(display, area);
    }
}

/// Word-wrap a single logical line to fit within `max_width` columns.
/// Returns a list of visual line segments. Tries to break at word boundaries;
/// falls back to hard breaks if a word is longer than the width.
fn wrap_line(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    if text.is_empty() {
        return vec![String::new()];
    }
    if text.len() <= max_width {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_width {
            lines.push(remaining.to_string());
            break;
        }

        // Find the last space within max_width to break at.
        let chunk = &remaining[..max_width];
        let break_pos = if let Some(pos) = chunk.rfind(' ') {
            // Don't break too early — at least half the width should be used.
            if pos > max_width / 3 {
                pos + 1 // Include the space on the current line.
            } else {
                max_width // Hard break.
            }
        } else {
            max_width // No space found — hard break.
        };

        // Ensure we don't split in the middle of a multi-byte char.
        let break_pos = remaining
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= break_pos)
            .last()
            .unwrap_or(break_pos);

        let (line, rest) = remaining.split_at(break_pos);
        lines.push(line.to_string());
        remaining = rest;
    }

    lines
}
