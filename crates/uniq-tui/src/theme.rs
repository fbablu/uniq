//! Color scheme and styling for the TUI.

use ratatui::style::{Color, Modifier, Style};

/// The color palette for uniq's TUI.
pub struct Theme;

impl Theme {
    // ── Base colors ─────────────────────────────────────────
    pub fn bg() -> Color {
        Color::Reset
    }

    pub fn fg() -> Color {
        Color::White
    }

    pub fn fg_dim() -> Color {
        Color::DarkGray
    }

    // ── Accent colors ───────────────────────────────────────
    pub fn accent() -> Color {
        Color::Cyan
    }

    pub fn accent_secondary() -> Color {
        Color::Magenta
    }

    pub fn success() -> Color {
        Color::Green
    }

    pub fn warning() -> Color {
        Color::Yellow
    }

    pub fn error() -> Color {
        Color::Red
    }

    // ── Phase tab colors ────────────────────────────────────
    pub fn phase_active() -> Color {
        Color::Cyan
    }

    pub fn phase_inactive() -> Color {
        Color::DarkGray
    }

    // ── Blend ratio bar colors ──────────────────────────────
    pub fn blend_a() -> Color {
        Color::Cyan
    }

    pub fn blend_b() -> Color {
        Color::Magenta
    }

    // ── Composite styles ────────────────────────────────────

    pub fn title() -> Style {
        Style::default()
            .fg(Self::accent())
            .add_modifier(Modifier::BOLD)
    }

    pub fn header() -> Style {
        Style::default().fg(Self::fg()).add_modifier(Modifier::BOLD)
    }

    pub fn selected() -> Style {
        Style::default()
            .fg(Self::accent())
            .add_modifier(Modifier::BOLD)
    }

    pub fn normal() -> Style {
        Style::default().fg(Self::fg())
    }

    pub fn dim() -> Style {
        Style::default().fg(Self::fg_dim())
    }

    pub fn status_bar() -> Style {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    }

    pub fn tab_active() -> Style {
        Style::default()
            .fg(Self::phase_active())
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }

    pub fn tab_inactive() -> Style {
        Style::default().fg(Self::phase_inactive())
    }

    pub fn score_color(score: f64, max: f64) -> Color {
        let ratio = score / max;
        if ratio >= 0.8 {
            Self::success()
        } else if ratio >= 0.5 {
            Self::warning()
        } else {
            Self::error()
        }
    }
}
