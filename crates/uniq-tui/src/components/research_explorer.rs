//! Phase 2: Research Discovery — search and display academic papers.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use crate::action::Action;
use crate::components::Component;
use crate::theme::Theme;

use uniq_core::research::PaperMeta;

/// Braille spinner frames.
const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct ResearchExplorerComponent {
    /// All discovered papers.
    pub papers: Vec<PaperMeta>,
    /// Currently selected paper index.
    pub selected: usize,
    /// Whether search is in progress.
    pub searching: bool,
    /// Progress (papers found so far vs target).
    pub progress: (usize, usize),
    /// Error message.
    pub error: Option<String>,
    /// Whether expanded detail view is open.
    detail_expanded: bool,
    /// Scroll position within the expanded detail view.
    detail_scroll: u16,
    /// Spinner animation frame counter.
    spinner_tick: usize,
    /// Current query being searched.
    current_query: String,
    /// Query-level progress: (current idx 0-based, total).
    query_progress: (usize, usize),
}

impl ResearchExplorerComponent {
    pub fn new() -> Self {
        Self {
            papers: Vec::new(),
            selected: 0,
            searching: false,
            progress: (0, 0),
            error: None,
            detail_expanded: false,
            detail_scroll: 0,
            spinner_tick: 0,
            current_query: String::new(),
            query_progress: (0, 0),
        }
    }
}

impl Component for ResearchExplorerComponent {
    fn handle_action(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::Tick => {
                if self.searching {
                    self.spinner_tick = self.spinner_tick.wrapping_add(1);
                }
                None
            }
            Action::Confirm => {
                if self.detail_expanded {
                    self.detail_expanded = false;
                    self.detail_scroll = 0;
                } else if !self.papers.is_empty() {
                    self.detail_expanded = true;
                    self.detail_scroll = 0;
                }
                None
            }
            Action::CloseMergeDialog => {
                if self.detail_expanded {
                    self.detail_expanded = false;
                    self.detail_scroll = 0;
                    return None;
                }
                None
            }
            Action::ScrollUp | Action::SelectPrev => {
                if self.detail_expanded {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                } else if self.selected > 0 {
                    self.selected -= 1;
                }
                None
            }
            Action::ScrollDown | Action::SelectNext => {
                if self.detail_expanded {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                } else if self.selected + 1 < self.papers.len() {
                    self.selected += 1;
                }
                None
            }
            Action::SearchQueryStarted {
                query,
                query_idx,
                total_queries,
            } => {
                self.current_query = query.clone();
                self.query_progress = (*query_idx, *total_queries);
                None
            }
            Action::PapersFound(papers) => {
                self.papers.extend(papers.clone());
                None
            }
            Action::ResearchComplete => {
                self.searching = false;
                self.current_query.clear();
                Some(Action::SetStatus(format!(
                    "Found {} papers. Press [Enter] to view, [Right] for next phase.",
                    self.papers.len()
                )))
            }
            Action::ResearchFailed(err) => {
                self.searching = false;
                self.error = Some(err.clone());
                None
            }
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Research Discovery ")
            .title_style(Theme::title())
            .borders(Borders::ALL)
            .border_style(Theme::dim());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Empty state.
        if self.papers.is_empty() && !self.searching {
            let msg = if let Some(ref err) = self.error {
                Paragraph::new(Span::styled(
                    format!("Error: {}", err),
                    Style::default().fg(Theme::error()),
                ))
            } else {
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled("No papers loaded yet.", Theme::dim())),
                    Line::from(Span::styled(
                        "Complete Phase 1 first, then research will begin automatically.",
                        Theme::dim(),
                    )),
                ])
            };
            frame.render_widget(msg, inner);
            return;
        }

        // Expanded detail view.
        if self.detail_expanded {
            if let Some(paper) = self.papers.get(self.selected) {
                self.render_expanded_detail(frame, inner, paper);
            }
            return;
        }

        // Searching: show spinner + progress + thinking text + any papers found so far.
        if self.searching {
            self.render_searching(frame, inner);
            return;
        }

        // Normal view: summary + table + detail.
        let chunks = Layout::vertical([
            Constraint::Length(2), // Summary
            Constraint::Min(10),   // Paper table
            Constraint::Length(8), // Paper detail
        ])
        .split(inner);

        // Summary line.
        let summary = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{} papers found", self.papers.len()),
                Theme::header(),
            ),
            Span::styled("  |  ", Theme::dim()),
            Span::styled("[Enter]", Theme::selected()),
            Span::styled(" view details  ", Theme::dim()),
            Span::styled("[Right]", Theme::selected()),
            Span::styled(" next phase", Theme::dim()),
        ]));
        frame.render_widget(summary, chunks[0]);

        self.render_paper_table(frame, chunks[1]);
        self.render_paper_detail(frame, chunks[2]);
    }
}

impl ResearchExplorerComponent {
    // ── Searching view ──────────────────────────────────────────

    fn render_searching(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::vertical([
            Constraint::Length(6), // Progress panel
            Constraint::Min(4),    // Papers found so far (if any)
        ])
        .split(area);

        let spinner = SPINNER[self.spinner_tick % SPINNER.len()];
        let w = chunks[0].width as usize;

        // Progress bar: visual bar made of block characters.
        let (qi, qt) = self.query_progress;
        let pct = if qt > 0 {
            ((qi + 1) as f64 / qt as f64).min(1.0)
        } else {
            0.0
        };
        let bar_width = w.saturating_sub(2);
        let filled = (bar_width as f64 * pct) as usize;
        let empty = bar_width.saturating_sub(filled);
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

        // Truncate query to fit.
        let query_display = truncate(&self.current_query, w.saturating_sub(4));

        let panel = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    format!(" {} ", spinner),
                    Style::default()
                        .fg(Theme::accent())
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::styled(
                    format!("Searching academic databases...  query {}/{}", qi + 1, qt),
                    Theme::header(),
                ),
            ]),
            Line::from(Span::styled(
                format!(" {}", bar),
                Style::default().fg(Theme::accent()),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("   → ", Style::default().fg(Theme::warning())),
                Span::styled(format!("\"{}\"", query_display), Theme::normal()),
            ]),
            Line::from(vec![Span::styled(
                format!("   {} papers found so far", self.papers.len()),
                Theme::dim(),
            )]),
        ]);
        frame.render_widget(panel, chunks[0]);

        // Show papers found so far in a mini table.
        if !self.papers.is_empty() {
            self.render_paper_table(frame, chunks[1]);
        } else {
            let waiting = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("   Waiting for results...", Theme::dim())),
            ]);
            frame.render_widget(waiting, chunks[1]);
        }
    }

    // ── Paper table ─────────────────────────────────────────────

    fn render_paper_table(&self, frame: &mut Frame, area: Rect) {
        let table_width = area.width as usize;
        let fixed_cols = 4 + 6 + 7 + 6 + 4;
        let title_max = table_width.saturating_sub(fixed_cols).max(10);

        let table_inner_height = area.height.saturating_sub(2) as usize;
        let scroll_offset = if self.selected >= table_inner_height {
            self.selected - table_inner_height + 1
        } else {
            0
        };

        let header = Row::new(vec![
            Cell::from(" # "),
            Cell::from("Title"),
            Cell::from(" Year"),
            Cell::from("  Cites"),
            Cell::from("Source"),
        ])
        .style(Theme::header());

        let rows: Vec<Row> = self
            .papers
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(table_inner_height)
            .map(|(i, paper)| {
                let style = if i == self.selected {
                    Theme::selected()
                } else {
                    Theme::normal()
                };

                let source_str = match paper.source {
                    uniq_core::research::PaperSource::SemanticScholar => "S2",
                    uniq_core::research::PaperSource::ArXiv => "arXiv",
                };

                Row::new(vec![
                    Cell::from(format!("{:>3}", i + 1)),
                    Cell::from(truncate(&paper.title, title_max)),
                    Cell::from(
                        paper
                            .year
                            .map(|y| format!(" {}", y))
                            .unwrap_or_else(|| "    ".to_string()),
                    ),
                    Cell::from(
                        paper
                            .citation_count
                            .map(|c| format!("{:>6}", c))
                            .unwrap_or_else(|| "     —".to_string()),
                    ),
                    Cell::from(format!("{:<5}", source_str)),
                ])
                .style(style)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Min(10),
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Length(6),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(Block::default().borders(Borders::TOP));

        frame.render_widget(table, area);
    }

    // ── Paper detail (compact) ──────────────────────────────────

    fn render_paper_detail(&self, frame: &mut Frame, area: Rect) {
        let Some(paper) = self.papers.get(self.selected) else {
            return;
        };

        let detail_block = Block::default()
            .title(" Paper Detail ")
            .borders(Borders::ALL)
            .border_style(Theme::dim());

        let detail_inner_width = area.width.saturating_sub(2) as usize;
        let abstract_text = truncate(&paper.abstract_text, detail_inner_width * 2);
        let authors_text = truncate(
            &paper.authors.join(", "),
            detail_inner_width.saturating_sub(10),
        );

        let source_str = match paper.source {
            uniq_core::research::PaperSource::SemanticScholar => "Semantic Scholar",
            uniq_core::research::PaperSource::ArXiv => "arXiv",
        };

        let pdf_label = if paper.pdf_url.is_some() {
            "Available"
        } else {
            "N/A"
        };
        let pdf_style = if paper.pdf_url.is_some() {
            Style::default().fg(Theme::success())
        } else {
            Theme::dim()
        };

        let detail = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Authors: ", Theme::header()),
                Span::styled(authors_text, Theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("Source: ", Theme::header()),
                Span::styled(source_str, Theme::normal()),
                Span::styled("  PDF: ", Theme::header()),
                Span::styled(pdf_label, pdf_style),
            ]),
            Line::from(""),
            Line::from(Span::styled(abstract_text, Theme::dim())),
        ])
        .wrap(Wrap { trim: true })
        .block(detail_block);

        frame.render_widget(detail, area);
    }

    // ── Expanded detail ─────────────────────────────────────────

    fn render_expanded_detail(&self, frame: &mut Frame, area: Rect, paper: &PaperMeta) {
        let block = Block::default()
            .title(format!(
                " Paper {}/{} — [Enter/Esc] close  [↑↓] scroll ",
                self.selected + 1,
                self.papers.len()
            ))
            .title_style(Theme::title())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::accent()));

        let inner = block.inner(area);
        let w = inner.width as usize;

        let source_str = match paper.source {
            uniq_core::research::PaperSource::SemanticScholar => "Semantic Scholar",
            uniq_core::research::PaperSource::ArXiv => "arXiv",
        };

        let pdf_label = if paper.pdf_url.is_some() {
            "Available"
        } else {
            "N/A"
        };

        let date_str = paper
            .published_date
            .map(|d| d.to_string())
            .unwrap_or_else(|| "—".to_string());

        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("Title: ", Theme::header()),
                Span::styled(&paper.title, Theme::normal()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Authors: ", Theme::header()),
                Span::styled(paper.authors.join(", "), Theme::normal()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Year: ", Theme::header()),
                Span::styled(
                    paper
                        .year
                        .map(|y| y.to_string())
                        .unwrap_or_else(|| "—".to_string()),
                    Theme::normal(),
                ),
                Span::styled("    Published: ", Theme::header()),
                Span::styled(date_str, Theme::normal()),
                Span::styled("    Citations: ", Theme::header()),
                Span::styled(
                    paper
                        .citation_count
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "—".to_string()),
                    Theme::normal(),
                ),
            ]),
            Line::from(vec![
                Span::styled("Source: ", Theme::header()),
                Span::styled(source_str, Theme::normal()),
                Span::styled("    PDF: ", Theme::header()),
                Span::styled(
                    pdf_label,
                    if paper.pdf_url.is_some() {
                        Style::default().fg(Theme::success())
                    } else {
                        Theme::dim()
                    },
                ),
            ]),
            Line::from(vec![
                Span::styled("URL: ", Theme::header()),
                Span::styled(&paper.url, Theme::dim()),
            ]),
        ];

        if let Some(ref pdf) = paper.pdf_url {
            lines.push(Line::from(vec![
                Span::styled("PDF: ", Theme::header()),
                Span::styled(pdf, Theme::dim()),
            ]));
        }

        if !paper.fields.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Fields: ", Theme::header()),
                Span::styled(paper.fields.join(", "), Theme::normal()),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Abstract", Theme::header())));
        lines.push(Line::from(Span::styled(
            "─".repeat(w.min(60)),
            Theme::dim(),
        )));
        lines.push(Line::from(""));

        for wrapped_line in word_wrap(&paper.abstract_text, w.saturating_sub(1).max(1)) {
            lines.push(Line::from(Span::styled(wrapped_line, Theme::normal())));
        }

        let para = Paragraph::new(lines)
            .scroll((self.detail_scroll, 0))
            .block(block);

        frame.render_widget(para, area);
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current_line = String::new();
        for word in paragraph.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= max_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
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
