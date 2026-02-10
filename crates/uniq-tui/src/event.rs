//! Terminal event handling — captures keyboard, mouse, and resize events
//! from crossterm and dispatches them as Actions.
//!
//! The handler operates in two modes:
//! - Normal: keys are mapped to global shortcuts (quit, navigate, scroll).
//! - Editing: keys are forwarded as raw CharInput/BackspaceInput so text
//!   fields can receive typed characters.
//!
//! The current InputMode is shared between the App and EventHandler via
//! an Arc<AtomicU8>.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::action::{Action, InputMode};

/// Encode InputMode as u8 for atomic sharing.
const MODE_NORMAL: u8 = 0;
const MODE_EDITING: u8 = 1;

/// Shared flag the App sets so the EventHandler knows which keymap to use.
pub type InputModeFlag = Arc<AtomicU8>;

pub fn new_input_mode_flag() -> InputModeFlag {
    Arc::new(AtomicU8::new(MODE_NORMAL))
}

pub fn set_input_mode(flag: &InputModeFlag, mode: InputMode) {
    let val = match mode {
        InputMode::Normal => MODE_NORMAL,
        InputMode::Editing => MODE_EDITING,
    };
    flag.store(val, Ordering::Relaxed);
}

fn get_input_mode(flag: &InputModeFlag) -> InputMode {
    match flag.load(Ordering::Relaxed) {
        MODE_EDITING => InputMode::Editing,
        _ => InputMode::Normal,
    }
}

/// Event loop that reads terminal events and sends Actions.
pub struct EventHandler {
    tx: mpsc::UnboundedSender<Action>,
    tick_rate: Duration,
    mode_flag: InputModeFlag,
}

impl EventHandler {
    pub fn new(
        tx: mpsc::UnboundedSender<Action>,
        tick_rate: Duration,
        mode_flag: InputModeFlag,
    ) -> Self {
        Self {
            tx,
            tick_rate,
            mode_flag,
        }
    }

    /// Run the event loop. This blocks and should be spawned in a task.
    pub async fn run(&self) {
        let mut interval = tokio::time::interval(self.tick_rate);

        loop {
            let action = tokio::select! {
                _ = interval.tick() => {
                    Some(Action::Tick)
                }
                result = tokio::task::spawn_blocking({
                    || {
                        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                            event::read().ok()
                        } else {
                            None
                        }
                    }
                }) => {
                    match result {
                        Ok(Some(event)) => self.map_event(event),
                        _ => None,
                    }
                }
            };

            if let Some(action) = action {
                if self.tx.send(action).is_err() {
                    break;
                }
            }
        }
    }

    fn map_event(&self, event: Event) -> Option<Action> {
        match event {
            Event::Key(key) => self.map_key(key),
            Event::Paste(text) => Some(Action::PasteBulk(text)),
            Event::Resize(_, _) => Some(Action::Tick),
            _ => None,
        }
    }

    fn map_key(&self, key: KeyEvent) -> Option<Action> {
        // Ctrl+C always quits regardless of mode.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Some(Action::Quit);
        }

        match get_input_mode(&self.mode_flag) {
            InputMode::Editing => self.map_key_editing(key),
            InputMode::Normal => self.map_key_normal(key),
        }
    }

    /// Key mapping when a text field is focused. Most keys become character
    /// input; only a few are reserved for navigation.
    fn map_key_editing(&self, key: KeyEvent) -> Option<Action> {
        // Ctrl shortcuts that work in editing mode.
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('w') => Some(Action::DeleteWord),
                KeyCode::Char('v') => Some(Action::PasteInput),
                KeyCode::Char('s') => Some(Action::SubmitForm),
                KeyCode::Enter => Some(Action::SubmitForm),
                _ => None,
            };
        }

        // Alt+Enter also submits (some terminals send this instead of Ctrl+Enter).
        if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Enter {
            return Some(Action::SubmitForm);
        }

        match key.code {
            // Escape exits editing mode and returns to normal.
            KeyCode::Esc => Some(Action::CloseMergeDialog), // re-used; App interprets contextually
            // Tab switches between input fields (not phase navigation).
            KeyCode::Tab | KeyCode::BackTab => Some(Action::SwitchInputField),
            // Enter inserts newline (multi-line) or navigates (single-line).
            // The component decides what to do based on which field is focused.
            KeyCode::Enter => Some(Action::NewlineInput),
            // Arrow up/down scroll / navigate.
            KeyCode::Up => Some(Action::ScrollUp),
            KeyCode::Down => Some(Action::ScrollDown),
            // Backspace deletes.
            KeyCode::Backspace => Some(Action::BackspaceInput),
            // Any printable character is forwarded.
            KeyCode::Char(c) => Some(Action::CharInput(c)),
            _ => None,
        }
    }

    /// Key mapping in normal mode — global shortcuts.
    fn map_key_normal(&self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Char('?') => Some(Action::ToggleHelp),
            KeyCode::Right | KeyCode::Tab => Some(Action::NextPhase),
            KeyCode::Left | KeyCode::BackTab => Some(Action::PrevPhase),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::ScrollUp),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::ScrollDown),
            KeyCode::Enter => Some(Action::Confirm),
            KeyCode::Char('m') => Some(Action::OpenMergeDialog),
            KeyCode::Esc => Some(Action::CloseMergeDialog),

            // Number keys for direct phase navigation.
            KeyCode::Char('1') => Some(Action::GoToPhase(crate::action::Phase::ProjectIntake)),
            KeyCode::Char('2') => Some(Action::GoToPhase(crate::action::Phase::ResearchDiscovery)),
            KeyCode::Char('3') => Some(Action::GoToPhase(crate::action::Phase::TechniqueSelection)),
            KeyCode::Char('4') => Some(Action::GoToPhase(crate::action::Phase::VariantGeneration)),
            KeyCode::Char('5') => Some(Action::GoToPhase(crate::action::Phase::Benchmarking)),

            _ => None,
        }
    }
}
