//! Color scheme and styling for the TUI.

use ratatui::style::{Color, Modifier, Style};

/// The color palette for uniq's TUI.
///
/// Uses RGB colors for a professional, subdued look inspired by
/// Claude Code, lazygit, and similar modern terminal UIs.
pub struct Theme;

impl Theme {
    // ── Base colors ─────────────────────────────────────────
    pub fn bg() -> Color {
        Color::Reset
    }

    pub fn fg() -> Color {
        Color::Rgb(200, 200, 200)
    }

    pub fn fg_dim() -> Color {
        Color::Rgb(100, 100, 100)
    }

    pub fn fg_muted() -> Color {
        Color::Rgb(140, 140, 140)
    }

    // ── Accent colors ───────────────────────────────────────
    pub fn accent() -> Color {
        Color::Rgb(110, 170, 255)
    }

    pub fn accent_secondary() -> Color {
        Color::Rgb(180, 130, 240)
    }

    pub fn success() -> Color {
        Color::Rgb(80, 200, 120)
    }

    pub fn warning() -> Color {
        Color::Rgb(230, 180, 80)
    }

    pub fn error() -> Color {
        Color::Rgb(240, 80, 80)
    }

    // ── Structural colors ───────────────────────────────────
    pub fn border_color() -> Color {
        Color::Rgb(60, 60, 60)
    }

    pub fn selection_bg() -> Color {
        Color::Rgb(40, 40, 60)
    }

    // ── Phase tab colors ────────────────────────────────────
    pub fn phase_active() -> Color {
        Self::accent()
    }

    pub fn phase_inactive() -> Color {
        Self::fg_dim()
    }

    pub fn phase_completed() -> Color {
        Self::success()
    }

    // ── Blend ratio bar colors ──────────────────────────────
    pub fn blend_a() -> Color {
        Self::accent()
    }

    pub fn blend_b() -> Color {
        Self::accent_secondary()
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

    pub fn muted() -> Style {
        Style::default().fg(Self::fg_muted())
    }

    pub fn border() -> Style {
        Style::default().fg(Self::border_color())
    }

    pub fn key_hint() -> Style {
        Style::default().fg(Self::accent())
    }

    pub fn selection() -> Style {
        Style::default().bg(Self::selection_bg())
    }

    pub fn status_bar() -> Style {
        Style::default().fg(Self::fg_muted())
    }

    pub fn tab_active() -> Style {
        Style::default()
            .fg(Self::phase_active())
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }

    pub fn tab_inactive() -> Style {
        Style::default().fg(Self::phase_inactive())
    }

    pub fn tab_completed() -> Style {
        Style::default()
            .fg(Self::phase_completed())
            .add_modifier(Modifier::DIM)
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
