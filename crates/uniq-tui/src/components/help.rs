//! Help overlay — keybinding reference.

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::action::Action;
use crate::components::Component;
use crate::theme::Theme;

pub struct HelpComponent {
    pub visible: bool,
}

impl HelpComponent {
    pub fn new() -> Self {
        Self { visible: false }
    }

    fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
        let vertical = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .flex(Flex::Center)
        .split(area);

        let horizontal = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .flex(Flex::Center)
        .split(vertical[1]);

        horizontal[1]
    }
}

impl Component for HelpComponent {
    fn handle_action(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::ToggleHelp => {
                self.visible = !self.visible;
                None
            }
            _ if self.visible => {
                // Any key closes help.
                self.visible = false;
                None
            }
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let dialog = Self::centered_rect(area, 55, 22);
        frame.render_widget(Clear, dialog);

        let block = Block::default()
            .title(" Help — Keybindings ")
            .title_style(Theme::title())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::accent()));

        let help_text = vec![
            Line::from(""),
            key_line("q / Ctrl+C", "Quit"),
            key_line("?", "Toggle this help"),
            key_line("1-5", "Jump to phase"),
            key_line("Left / Right", "Previous / next phase"),
            key_line("Tab / Shift+Tab", "Next / previous phase"),
            key_line("Up / Down / j / k", "Scroll / select"),
            key_line("Enter", "Confirm / toggle"),
            key_line("m", "Open merge dialog"),
            key_line("Esc", "Close dialog"),
            Line::from(""),
            Line::from(Span::styled("── Phase-specific ──", Theme::header())),
            Line::from(""),
            key_line("Phase 1", "Enter path & description, then Enter"),
            key_line("Phase 2", "Auto-searches after Phase 1"),
            key_line("Phase 3", "Space/Enter to toggle technique selection"),
            key_line("Phase 4", "Auto-generates after Phase 3"),
            key_line("Phase 5", "View benchmarks, rate variants"),
        ];

        let paragraph = Paragraph::new(help_text).block(block);
        frame.render_widget(paragraph, dialog);
    }
}

fn key_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {:<22}", key), Theme::selected()),
        Span::styled(desc, Theme::normal()),
    ])
}
