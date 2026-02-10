//! Status bar at the bottom of the TUI.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::action::{Action, Phase};
use crate::components::Component;
use crate::theme::Theme;

pub struct StatusBarComponent {
    /// Current status message.
    pub message: String,
    /// Current active phase.
    pub current_phase: Phase,
}

impl StatusBarComponent {
    pub fn new() -> Self {
        Self {
            message: "Welcome to uniq. Set up your project in Phase 1.".to_string(),
            current_phase: Phase::ProjectIntake,
        }
    }
}

impl Component for StatusBarComponent {
    fn handle_action(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::SetStatus(msg) => {
                self.message = msg.clone();
                None
            }
            Action::ClearStatus => {
                self.message.clear();
                None
            }
            Action::GoToPhase(phase) => {
                self.current_phase = *phase;
                None
            }
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let width = area.width as usize;

        // Right side: key hints (fixed width).
        let hints = " [q]uit [?]help [1-5]phase [m]erge";
        let hints_len = hints.len();

        // Phase label.
        let label = format!(" {} ", self.current_phase.label());
        let label_len = label.len();

        // Truncate message to remaining space.
        let msg_budget = width
            .saturating_sub(label_len)
            .saturating_sub(hints_len)
            .saturating_sub(2); // 2 for padding

        let msg = if self.message.len() > msg_budget {
            if msg_budget > 3 {
                format!("{}...", &self.message[..msg_budget - 3])
            } else {
                String::new()
            }
        } else {
            self.message.clone()
        };

        // Pad to push hints to the right edge.
        let used = label_len + 1 + msg.len();
        let pad = width.saturating_sub(used + hints_len);

        let line = Line::from(vec![
            Span::styled(label, Theme::status_bar()),
            Span::styled(" ", Theme::dim()),
            Span::styled(msg, Theme::dim()),
            Span::raw(" ".repeat(pad)),
            Span::styled("[q]", Theme::selected()),
            Span::styled("uit ", Theme::dim()),
            Span::styled("[?]", Theme::selected()),
            Span::styled("help ", Theme::dim()),
            Span::styled("[1-5]", Theme::selected()),
            Span::styled("phase ", Theme::dim()),
            Span::styled("[m]", Theme::selected()),
            Span::styled("erge", Theme::dim()),
        ]);

        frame.render_widget(Paragraph::new(line), area);
    }
}
