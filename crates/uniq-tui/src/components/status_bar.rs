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

    /// Short phase name for the pill badge.
    fn phase_badge(&self) -> &'static str {
        match self.current_phase {
            Phase::ProjectIntake => "Intake",
            Phase::ResearchDiscovery => "Research",
            Phase::TechniqueSelection => "Techniques",
            Phase::VariantGeneration => "Build",
            Phase::Benchmarking => "Benchmark",
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

        // Right side: compact key hints
        let hints = "q·?·1-5·m";
        let hints_len = hints.len() + 1; // +1 for trailing space

        // Phase badge
        let badge = self.phase_badge();
        let badge_len = badge.len() + 2; // spaces around badge

        // Truncate message to remaining space
        let msg_budget = width
            .saturating_sub(badge_len)
            .saturating_sub(hints_len)
            .saturating_sub(4); // separators and spacing

        let msg = if self.message.len() > msg_budget {
            if msg_budget > 3 {
                format!("{}...", &self.message[..msg_budget - 3])
            } else {
                String::new()
            }
        } else {
            self.message.clone()
        };

        // Pad to push hints to the right edge
        let used = badge_len + 2 + msg.len();
        let pad = width.saturating_sub(used + hints_len);

        let line = Line::from(vec![
            Span::styled(format!(" {} ", badge), Theme::muted()),
            Span::styled("  ", Theme::dim()),
            Span::styled(msg, Theme::dim()),
            Span::raw(" ".repeat(pad)),
            Span::styled(hints, Theme::key_hint()),
            Span::raw(" "),
        ]);

        frame.render_widget(Paragraph::new(line), area);
    }
}
