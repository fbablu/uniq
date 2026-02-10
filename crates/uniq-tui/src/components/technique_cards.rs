//! Phase 3: Technique Selection — view technique cards extracted from papers
//! and select which ones to generate variants for.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
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
    /// Progress: (done, total papers).
    pub progress: (usize, usize),
    /// Errors during extraction.
    pub errors: Vec<(String, String)>,
    /// Spinner animation frame counter.
    spinner_tick: usize,
    /// Title of the paper currently being extracted.
    current_paper: String,
    /// Titles of papers being processed concurrently.
    active_papers: Vec<String>,
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
                self.active_papers.push(paper_title.clone());
                // Keep only the most recent few for display.
                if self.active_papers.len() > 5 {
                    self.active_papers.remove(0);
                }
                None
            }
            Action::TechniqueExtracted(card) => {
                self.progress.0 += 1;
                // Remove from active list.
                self.active_papers.retain(|t| t != &card.paper_title);
                self.techniques.push(*card.clone());
                self.techniques
                    .sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());
                None
            }
            Action::TechniqueExtractionFailed { paper_id, error } => {
                self.progress.0 += 1;
                // Remove from active list by paper_id prefix match.
                self.active_papers
                    .retain(|t| !paper_id.contains(t) && !t.contains(paper_id));
                self.errors.push((paper_id.clone(), error.clone()));
                None
            }
            Action::ExtractionComplete => {
                self.extracting = false;
                self.active_papers.clear();
                self.current_paper.clear();
                // Auto-select top 10.
                for (i, tech) in self.techniques.iter_mut().enumerate() {
                    tech.selected = i < 10;
                }
                Some(Action::SetStatus(format!(
                    "Extracted {} techniques. Select and press [Right] to generate variants.",
                    self.techniques.len()
                )))
            }
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Technique Selection ")
            .title_style(Theme::title())
            .borders(Borders::ALL)
            .border_style(Theme::dim());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Empty state (not extracting).
        if self.techniques.is_empty() && !self.extracting {
            let mut lines = vec![Line::from("")];
            if !self.errors.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("Extraction failed for all {} papers.", self.errors.len()),
                    Style::default().fg(Theme::error()),
                )));
                if let Some((_id, err)) = self.errors.first() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(truncate(err, 120), Theme::dim())));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Hint: Set ANTHROPIC_API_KEY to enable technique extraction.",
                    Theme::dim(),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "No techniques extracted yet.",
                    Theme::dim(),
                )));
                lines.push(Line::from(Span::styled(
                    "Complete Phase 2 first, then extraction will begin automatically.",
                    Theme::dim(),
                )));
            }
            let msg = Paragraph::new(lines);
            frame.render_widget(msg, inner);
            return;
        }

        // Extracting: show progress panel + any techniques extracted so far.
        if self.extracting {
            self.render_extracting(frame, inner);
            return;
        }

        // Normal view: summary + list + detail.
        let chunks = Layout::vertical([
            Constraint::Length(2),  // Summary
            Constraint::Min(10),    // Technique list
            Constraint::Length(10), // Detail panel
        ])
        .split(inner);

        // Summary.
        let summary = Paragraph::new(Line::from(vec![
            Span::styled(
                format!(
                    "{}/{} techniques selected",
                    self.selected_count(),
                    self.techniques.len()
                ),
                Theme::header(),
            ),
            Span::styled("  |  ", Theme::dim()),
            Span::styled("[Enter]", Theme::selected()),
            Span::styled(" toggle  ", Theme::dim()),
            Span::styled("[Right]", Theme::selected()),
            Span::styled(" generate variants", Theme::dim()),
        ]));
        frame.render_widget(summary, chunks[0]);

        self.render_technique_list(frame, chunks[1]);
        self.render_technique_detail(frame, chunks[2]);
    }
}

impl TechniqueCardsComponent {
    // ── Extraction progress view ────────────────────────────────

    fn render_extracting(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::vertical([
            Constraint::Length(9), // Progress panel
            Constraint::Min(4),    // Techniques found so far
        ])
        .split(area);

        let spinner = SPINNER[self.spinner_tick % SPINNER.len()];
        let w = chunks[0].width as usize;
        let (done, total) = self.progress;

        // Progress bar.
        let pct = if total > 0 {
            (done as f64 / total as f64).min(1.0)
        } else {
            0.0
        };
        let bar_width = w.saturating_sub(2);
        let filled = (bar_width as f64 * pct) as usize;
        let empty = bar_width.saturating_sub(filled);
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    format!(" {} ", spinner),
                    Style::default()
                        .fg(Theme::accent())
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::styled(
                    format!("Extracting techniques from papers...  {}/{}", done, total),
                    Theme::header(),
                ),
            ]),
            Line::from(Span::styled(
                format!(" {}", bar),
                Style::default().fg(Theme::accent()),
            )),
            Line::from(""),
        ];

        // Show active papers being processed.
        if !self.active_papers.is_empty() {
            for (i, title) in self.active_papers.iter().rev().take(3).enumerate() {
                let prefix = if i == 0 { "→" } else { " " };
                let style = if i == 0 {
                    Style::default().fg(Theme::warning())
                } else {
                    Theme::dim()
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", prefix), style),
                    Span::styled(
                        format!("\"{}\"", truncate(title, w.saturating_sub(8))),
                        if i == 0 {
                            Theme::normal()
                        } else {
                            Theme::dim()
                        },
                    ),
                ]));
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled("   → ", Style::default().fg(Theme::warning())),
                Span::styled(
                    format!("\"{}\"", truncate(&self.current_paper, w.saturating_sub(8))),
                    Theme::normal(),
                ),
            ]));
        }

        // Stats.
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                "   {} extracted, {} failed",
                self.techniques.len(),
                self.errors.len()
            ),
            Theme::dim(),
        )));

        let panel = Paragraph::new(lines);
        frame.render_widget(panel, chunks[0]);

        // Show techniques found so far.
        if !self.techniques.is_empty() {
            self.render_technique_list(frame, chunks[1]);
        } else {
            let waiting = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "   Waiting for first extraction to complete...",
                    Theme::dim(),
                )),
            ]);
            frame.render_widget(waiting, chunks[1]);
        }
    }

    // ── Technique list ──────────────────────────────────────────

    fn render_technique_list(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .techniques
            .iter()
            .enumerate()
            .map(|(i, tech)| {
                let checkbox = if tech.selected { "[x]" } else { "[ ]" };
                let relevance = format!("{:.0}%", tech.relevance_score * 100.0);
                let style = if i == self.selected {
                    Theme::selected()
                } else if tech.selected {
                    Style::default().fg(Theme::success())
                } else {
                    Theme::normal()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", checkbox), style),
                    Span::styled(format!("{:<40} ", truncate(&tech.name, 40)), style),
                    Span::styled(format!("{:<8} ", tech.implementation_complexity), style),
                    Span::styled(relevance, style),
                ]))
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::TOP));
        frame.render_widget(list, area);
    }

    // ── Technique detail ────────────────────────────────────────

    fn render_technique_detail(&self, frame: &mut Frame, area: Rect) {
        let Some(tech) = self.techniques.get(self.selected) else {
            return;
        };

        let detail_block = Block::default()
            .title(format!(" {} ", truncate(&tech.name, 50)))
            .title_style(Theme::title())
            .borders(Borders::ALL)
            .border_style(Theme::dim());

        let detail = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Paper: ", Theme::header()),
                Span::styled(&tech.paper_title, Theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("Components: ", Theme::header()),
                Span::styled(tech.key_components.join(", "), Theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("Dependencies: ", Theme::header()),
                Span::styled(tech.dependencies.join(", "), Theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("Complexity: ", Theme::header()),
                Span::styled(
                    format!("{}", tech.implementation_complexity),
                    Theme::normal(),
                ),
                Span::styled("  Hardware: ", Theme::header()),
                Span::styled(&tech.hardware_requirements, Theme::normal()),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                truncate(&tech.integration_approach, 200),
                Theme::dim(),
            )),
        ])
        .wrap(Wrap { trim: true })
        .block(detail_block);

        frame.render_widget(detail, area);
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
