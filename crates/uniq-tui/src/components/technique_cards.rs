//! Phase 3: Technique Selection — view technique cards extracted from papers
//! and select which ones to generate variants for.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::action::Action;
use crate::components::Component;
use crate::theme::Theme;

use uniq_core::research::TechniqueCard;

/// Braille spinner frames.
const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct TechniqueCardsComponent {
    /// All extracted technique cards.
    pub techniques: Vec<TechniqueCard>,
    /// Currently highlighted technique.
    pub selected: usize,
    /// Whether extraction is in progress.
    pub extracting: bool,
    /// Whether extraction was already attempted (prevents re-trigger loops).
    pub extraction_attempted: bool,
    /// Progress: (done, total).
    pub progress: (usize, usize),
    /// Errors during extraction.
    pub errors: Vec<(String, String)>,
    /// Spinner animation frame counter.
    spinner_tick: usize,
    /// Title of the paper currently being extracted.
    current_paper: String,
    /// Titles of papers being processed concurrently.
    pub active_papers: Vec<String>,
}

impl TechniqueCardsComponent {
    pub fn new() -> Self {
        Self {
            techniques: Vec::new(),
            selected: 0,
            extracting: false,
            extraction_attempted: false,
            progress: (0, 0),
            errors: Vec::new(),
            spinner_tick: 0,
            current_paper: String::new(),
            active_papers: Vec::new(),
        }
    }

    /// Get the number of selected techniques.
    pub fn selected_count(&self) -> usize {
        self.techniques.iter().filter(|t| t.selected).count()
    }
}

impl Component for TechniqueCardsComponent {
    fn handle_action(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::Tick => {
                if self.extracting {
                    self.spinner_tick = self.spinner_tick.wrapping_add(1);
                }
                None
            }
            Action::ScrollUp | Action::SelectPrev => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                None
            }
            Action::ScrollDown | Action::SelectNext => {
                if self.selected + 1 < self.techniques.len() {
                    self.selected += 1;
                }
                None
            }
            Action::ToggleTechnique(idx) => {
                if let Some(tech) = self.techniques.get_mut(*idx) {
                    tech.selected = !tech.selected;
                }
                None
            }
            Action::Confirm => {
                // Toggle the currently selected technique.
                if let Some(tech) = self.techniques.get_mut(self.selected) {
                    tech.selected = !tech.selected;
                }
                None
            }
            Action::ExtractionStarted { paper_title } => {
                self.current_paper = paper_title.clone();
                None
            }
            Action::TechniqueExtracted(card) => {
                self.techniques.push(*card.clone());
                self.techniques
                    .sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());
                None
            }
            Action::TechniqueExtractionFailed { paper_id, error } => {
                self.errors.push((paper_id.clone(), error.clone()));
                None
            }
            Action::ExtractionComplete => {
                self.extracting = false;
                self.active_papers.clear();
                self.current_paper.clear();
                // Auto-select all techniques (batch already filtered to top N).
                for tech in self.techniques.iter_mut() {
                    tech.selected = true;
                }
                Some(Action::SetStatus(format!(
                    "{} techniques found. Toggle with Enter, then → to generate variants.",
                    self.techniques.len()
                )))
            }
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        // No outer border — just use the space directly.
        // Empty state (not extracting).
        if self.techniques.is_empty() && !self.extracting {
            let mut lines = vec![Line::from(""), Line::from("")];
            if !self.errors.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("  Extraction failed for {} paper(s).", self.errors.len()),
                    Style::default().fg(Theme::error()),
                )));
                if let Some((_id, err)) = self.errors.first() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        format!("  {}", truncate(err, 100)),
                        Theme::dim(),
                    )));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "  Check ANTHROPIC_API_KEY and try again.",
                    Theme::muted(),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "  No techniques extracted yet.",
                    Theme::muted(),
                )));
                lines.push(Line::from(Span::styled(
                    "  Complete Phase 2 first, then extraction begins automatically.",
                    Theme::dim(),
                )));
            }
            frame.render_widget(Paragraph::new(lines), area);
            return;
        }

        // Extracting: show a clean progress view.
        if self.extracting {
            self.render_extracting(frame, area);
            return;
        }

        // Normal view: technique list + detail.
        let chunks = Layout::vertical([
            Constraint::Length(1), // Header
            Constraint::Min(8),    // Technique list
            Constraint::Length(9), // Detail panel
        ])
        .split(area);

        // Header.
        let header = Line::from(vec![
            Span::styled("  ", Theme::dim()),
            Span::styled(
                format!("{}/{}", self.selected_count(), self.techniques.len()),
                Theme::header(),
            ),
            Span::styled(" selected", Theme::muted()),
            Span::styled("    ", Theme::dim()),
            Span::styled("enter", Theme::key_hint()),
            Span::styled(" toggle  ", Theme::dim()),
            Span::styled("→", Theme::key_hint()),
            Span::styled(" generate variants", Theme::dim()),
        ]);
        frame.render_widget(Paragraph::new(header), chunks[0]);

        self.render_technique_list(frame, chunks[1]);
        self.render_technique_detail(frame, chunks[2]);
    }
}

impl TechniqueCardsComponent {
    // ── Extraction progress view ────────────────────────────

    fn render_extracting(&self, frame: &mut Frame, area: Rect) {
        let spinner = SPINNER[self.spinner_tick % SPINNER.len()];
        let elapsed_secs = self.spinner_tick / 10;

        let lines = vec![
            Line::from(""),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!("  {} ", spinner),
                    Style::default()
                        .fg(Theme::accent())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Analyzing paper abstracts with Claude", Theme::header()),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "  Extracting techniques from {} papers...  ({}s)",
                    self.progress.1.max(1) * 8, // approximate paper count
                    elapsed_secs
                ),
                Theme::muted(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  This uses abstracts instead of PDFs — typically 10-20 seconds.",
                Theme::dim(),
            )),
            Line::from(""),
            if !self.techniques.is_empty() {
                Line::from(Span::styled(
                    format!("  {} techniques found so far", self.techniques.len()),
                    Style::default().fg(Theme::success()),
                ))
            } else {
                Line::from(Span::styled("  Waiting for results...", Theme::dim()))
            },
        ];

        frame.render_widget(Paragraph::new(lines), area);
    }

    // ── Technique list ──────────────────────────────────────

    fn render_technique_list(&self, frame: &mut Frame, area: Rect) {
        let visible_height = area.height as usize;
        let scroll_offset = if self.selected >= visible_height {
            self.selected - visible_height + 1
        } else {
            0
        };

        let mut lines: Vec<Line> = Vec::new();
        for (i, tech) in self
            .techniques
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_height)
        {
            let is_selected = i == self.selected;
            let checkbox = if tech.selected { "◉" } else { "○" };
            let checkbox_style = if tech.selected {
                Style::default().fg(Theme::success())
            } else {
                Theme::dim()
            };

            let relevance = format!("{:.0}%", tech.relevance_score * 100.0);
            let complexity_style = match tech.implementation_complexity.to_string().as_str() {
                "Low" => Style::default().fg(Theme::success()),
                "High" => Style::default().fg(Theme::warning()),
                _ => Theme::muted(),
            };

            let row_style = if is_selected {
                Style::default().fg(Theme::fg()).bg(Theme::selection_bg())
            } else {
                Style::default()
            };

            let name_width = (area.width as usize).saturating_sub(20);
            lines.push(Line::from(vec![
                Span::styled(if is_selected { " ▸ " } else { "   " }, row_style),
                Span::styled(format!("{} ", checkbox), checkbox_style),
                Span::styled(
                    format!(
                        "{:<width$}",
                        truncate(&tech.name, name_width),
                        width = name_width
                    ),
                    if is_selected {
                        Style::default()
                            .fg(Theme::fg())
                            .bg(Theme::selection_bg())
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Theme::normal()
                    },
                ),
                Span::styled(
                    format!("{:<4}", tech.implementation_complexity),
                    complexity_style,
                ),
                Span::styled(format!(" {:>3}", relevance), Theme::muted()),
            ]));
        }

        frame.render_widget(Paragraph::new(lines), area);
    }

    // ── Technique detail ────────────────────────────────────

    fn render_technique_detail(&self, frame: &mut Frame, area: Rect) {
        let Some(tech) = self.techniques.get(self.selected) else {
            return;
        };

        let detail_block = Block::default()
            .borders(Borders::TOP)
            .border_style(Theme::border());

        let inner = detail_block.inner(area);
        frame.render_widget(detail_block, area);

        let detail = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("  Paper  ", Theme::muted()),
                Span::styled(&tech.paper_title, Theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("  Deps   ", Theme::muted()),
                Span::styled(tech.dependencies.join(", "), Theme::dim()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  ", Theme::dim()),
                Span::styled(
                    truncate(&tech.methodology, (inner.width as usize).saturating_sub(4)),
                    Theme::dim(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  ", Theme::dim()),
                Span::styled(
                    truncate(
                        &tech.integration_approach,
                        (inner.width as usize).saturating_sub(4),
                    ),
                    Theme::muted(),
                ),
            ]),
        ])
        .wrap(Wrap { trim: true });

        frame.render_widget(detail, inner);
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if max_len < 4 {
        return s.chars().take(max_len).collect();
    }
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
