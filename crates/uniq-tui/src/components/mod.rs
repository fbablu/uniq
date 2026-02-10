//! Component trait and all TUI components.
//!
//! Each component encapsulates rendering and input handling for a phase.

pub mod benchmark_dashboard;
pub mod help;
pub mod merge_dialog;
pub mod project_intake;
pub mod research_explorer;
pub mod status_bar;
pub mod technique_cards;
pub mod variant_builder;

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::action::Action;

/// Trait implemented by all TUI components.
pub trait Component {
    /// Handle an action and optionally return a new action to dispatch.
    fn handle_action(&mut self, action: &Action) -> Option<Action> {
        let _ = action;
        None
    }

    /// Render the component into the given area.
    fn render(&self, frame: &mut Frame, area: Rect);
}
