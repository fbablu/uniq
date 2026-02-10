//! Phase 2: Research Discovery — search and display academic papers.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
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
                    "{} papers found. Enter to view, → for next phase.",
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
        // Empty state.
        if self.papers.is_empty() && !self.searching {
            let msg = if let Some(ref err) = self.error {
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  Error: {}", err),
                        Style::default().fg(Theme::error()),
                    )),
                ])
            } else {
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(""),
                    Line::from(Span::styled("  No papers loaded yet.", Theme::muted())),
                    Line::from(Span::styled("  Complete Phase 1 first.", Theme::dim())),
                ])
            };
            frame.render_widget(msg, area);
            return;
        }

        // Expanded detail view.
        if self.detail_expanded {
            if let Some(paper) = self.papers.get(self.selected) {
                self.render_expanded_detail(frame, area, paper);
            }
            return;
        }

        // Searching view.
        if self.searching {
            self.render_searching(frame, area);
            return;
        }

        // Normal view: header + table + detail.
        let chunks = Layout::vertical([
            Constraint::Length(1), // Header
            Constraint::Min(8),    // Paper list
            Constraint::Length(7), // Paper detail
        ])
        .split(area);

        // Header.
        let header = Line::from(vec![
            Span::styled("  ", Theme::dim()),
            Span::styled(format!("{}", self.papers.len()), Theme::header()),
            Span::styled(" papers", Theme::muted()),
            Span::styled("    ", Theme::dim()),
            Span::styled("enter", Theme::key_hint()),
            Span::styled(" details  ", Theme::dim()),
            Span::styled("→", Theme::key_hint()),
            Span::styled(" next phase", Theme::dim()),
        ]);
        frame.render_widget(Paragraph::new(header), chunks[0]);

        self.render_paper_list(frame, chunks[1]);
        self.render_paper_detail(frame, chunks[2]);
    }
}

impl ResearchExplorerComponent {
    // ── Searching view ──────────────────────────────────────

    fn render_searching(&self, frame: &mut Frame, area: Rect) {
        let spinner = SPINNER[self.spinner_tick % SPINNER.len()];
        let elapsed_secs = self.spinner_tick / 10;

        let mut lines = vec![
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
                Span::styled("Searching Semantic Scholar + arXiv", Theme::header()),
                Span::styled(format!("  ({}s)", elapsed_secs), Theme::dim()),
            ]),
            Line::from(""),
        ];

        if !self.papers.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  {} papers found so far", self.papers.len()),
                Style::default().fg(Theme::success()),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  Searching academic databases...",
                Theme::muted(),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Typically takes 15-25 seconds.",
            Theme::dim(),
        )));

        frame.render_widget(Paragraph::new(lines), area);

        // Show papers found so far below the progress.
        if !self.papers.is_empty() {
            let list_area = Rect {
                y: area.y + 9,
                height: area.height.saturating_sub(9),
                ..area
            };
            if list_area.height > 2 {
                self.render_paper_list(frame, list_area);
            }
        }
    }

    // ── Paper list ──────────────────────────────────────────

    fn render_paper_list(&self, frame: &mut Frame, area: Rect) {
        let visible_height = area.height as usize;
        let scroll_offset = if self.selected >= visible_height {
            self.selected - visible_height + 1
        } else {
            0
        };

        let w = area.width as usize;
        let fixed_cols = 6 + 6 + 7 + 6; // num + year + cites + source
        let title_max = w.saturating_sub(fixed_cols).max(10);

        let mut lines: Vec<Line> = Vec::new();
        for (i, paper) in self
            .papers
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_height)
        {
            let is_selected = i == self.selected;
            let source_str = match paper.source {
                uniq_core::research::PaperSource::SemanticScholar => "S2",
                uniq_core::research::PaperSource::ArXiv => "arXiv",
            };

            let row_style = if is_selected {
                Style::default().fg(Theme::fg()).bg(Theme::selection_bg())
            } else {
                Style::default()
            };

            lines.push(Line::from(vec![
                Span::styled(if is_selected { " ▸ " } else { "   " }, row_style),
                Span::styled(
                    format!(
                        "{:<width$}",
                        truncate(&paper.title, title_max),
                        width = title_max
                    ),
                    if is_selected {
                        Style::default().fg(Theme::fg()).bg(Theme::selection_bg())
                    } else {
                        Theme::normal()
                    },
                ),
                Span::styled(
                    paper
                        .year
                        .map(|y| format!(" {}", y))
                        .unwrap_or_else(|| "     ".to_string()),
                    Theme::dim(),
                ),
                Span::styled(
                    paper
                        .citation_count
                        .map(|c| format!(" {:>5}", c))
                        .unwrap_or_else(|| "     —".to_string()),
                    Theme::muted(),
                ),
                Span::styled(format!("  {}", source_str), Theme::dim()),
            ]));
        }

        frame.render_widget(Paragraph::new(lines), area);
    }

    // ── Paper detail (compact) ──────────────────────────────

    fn render_paper_detail(&self, frame: &mut Frame, area: Rect) {
        let Some(paper) = self.papers.get(self.selected) else {
            return;
        };

        let detail_block = Block::default()
            .borders(Borders::TOP)
            .border_style(Theme::border());

        let inner = detail_block.inner(area);
        frame.render_widget(detail_block, area);

        let w = inner.width as usize;
        let authors_text = truncate(&paper.authors.join(", "), w.saturating_sub(12));
        let abstract_text = truncate(&paper.abstract_text, w * 2);

        let pdf_label = if paper.pdf_url.is_some() { "yes" } else { "no" };

        let detail = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("  Authors  ", Theme::muted()),
                Span::styled(authors_text, Theme::dim()),
            ]),
            Line::from(vec![
                Span::styled("  PDF      ", Theme::muted()),
                Span::styled(
                    pdf_label,
                    if paper.pdf_url.is_some() {
                        Style::default().fg(Theme::success())
                    } else {
                        Theme::dim()
                    },
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  ", Theme::dim()),
                Span::styled(abstract_text, Theme::dim()),
            ]),
        ])
        .wrap(Wrap { trim: true });

        frame.render_widget(detail, inner);
    }

    // ── Expanded detail ─────────────────────────────────────

    fn render_expanded_detail(&self, frame: &mut Frame, area: Rect, paper: &PaperMeta) {
        let block = Block::default()
            .title(format!(" {}/{} ", self.selected + 1, self.papers.len()))
            .title_style(Theme::muted())
            .borders(Borders::ALL)
            .border_style(Theme::border());

        let inner = block.inner(area);
        let w = inner.width as usize;

        let source_str = match paper.source {
            uniq_core::research::PaperSource::SemanticScholar => "Semantic Scholar",
            uniq_core::research::PaperSource::ArXiv => "arXiv",
        };

        let date_str = paper
            .published_date
            .map(|d| d.to_string())
            .unwrap_or_else(|| "—".to_string());

        let mut lines: Vec<Line> = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  ", Theme::dim()),
                Span::styled(&paper.title, Theme::header()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Authors   ", Theme::muted()),
                Span::styled(paper.authors.join(", "), Theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("  Year      ", Theme::muted()),
                Span::styled(
                    paper
                        .year
                        .map(|y| y.to_string())
                        .unwrap_or_else(|| "—".to_string()),
                    Theme::normal(),
                ),
                Span::styled("  Published  ", Theme::muted()),
                Span::styled(date_str, Theme::normal()),
                Span::styled("  Citations  ", Theme::muted()),
                Span::styled(
                    paper
                        .citation_count
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "—".to_string()),
                    Theme::normal(),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Source    ", Theme::muted()),
                Span::styled(source_str, Theme::normal()),
            ]),
            Line::from(vec![
                Span::styled("  URL       ", Theme::muted()),
                Span::styled(&paper.url, Theme::dim()),
            ]),
        ];

        if !paper.fields.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  Fields    ", Theme::muted()),
                Span::styled(paper.fields.join(", "), Theme::dim()),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {}", "─".repeat(w.saturating_sub(4).min(60))),
            Theme::border(),
        )));
        lines.push(Line::from(""));

        for wrapped_line in word_wrap(&paper.abstract_text, w.saturating_sub(4).max(1)) {
            lines.push(Line::from(Span::styled(
                format!("  {}", wrapped_line),
                Theme::normal(),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  enter/esc close  ↑↓ scroll",
            Theme::dim(),
        )));

        let para = Paragraph::new(lines)
            .scroll((self.detail_scroll, 0))
            .block(block);

        frame.render_widget(para, area);
    }
}

// ── Helpers ─────────────────────────────────────────────────

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
