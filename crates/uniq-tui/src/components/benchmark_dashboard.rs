//! Phase 5: Benchmark Dashboard — compare variants with multiple metrics.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use crate::action::Action;
use crate::components::Component;
use crate::theme::Theme;

use uniq_core::variant::Variant;

pub struct BenchmarkDashboardComponent {
    /// Reference to all variants (shared with VariantBuilder).
    pub variants: Vec<Variant>,
    /// Currently selected variant.
    pub selected: usize,
    /// Whether benchmarking is in progress.
    pub benchmarking: bool,
}

impl BenchmarkDashboardComponent {
    pub fn new() -> Self {
        Self {
            variants: Vec::new(),
            selected: 0,
            benchmarking: false,
        }
    }
}

impl Component for BenchmarkDashboardComponent {
    fn handle_action(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::ScrollUp | Action::SelectPrev => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                None
            }
            Action::ScrollDown | Action::SelectNext => {
                if self.selected + 1 < self.variants.len() {
                    self.selected += 1;
                }
                None
            }
            Action::BenchmarkComplete => {
                self.benchmarking = false;
                Some(Action::SetStatus("Benchmarking complete!".to_string()))
            }
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Benchmark Dashboard ")
            .title_style(Theme::title())
            .borders(Borders::ALL)
            .border_style(Theme::dim());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.variants.is_empty() {
            let msg = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("No variants to benchmark.", Theme::dim())),
                Line::from(Span::styled(
                    "Generate variants in Phase 4 first.",
                    Theme::dim(),
                )),
            ]);
            frame.render_widget(msg, inner);
            return;
        }

        let chunks = Layout::vertical([
            Constraint::Length(2),  // Summary bar
            Constraint::Min(8),     // Score table
            Constraint::Length(10), // Bar chart visualization
            Constraint::Length(8),  // Detail panel
        ])
        .split(inner);

        // Summary
        let benchmarked_count = self
            .variants
            .iter()
            .filter(|v| v.benchmark_results.is_some())
            .count();
        let summary = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{}/{} benchmarked", benchmarked_count, self.variants.len()),
                Theme::header(),
            ),
            Span::styled("  |  ", Theme::dim()),
            Span::styled("[m]", Theme::selected()),
            Span::styled("erge  ", Theme::dim()),
            Span::styled("[r]", Theme::selected()),
            Span::styled("un benchmark  ", Theme::dim()),
            Span::styled("[Enter]", Theme::selected()),
            Span::styled(" rate variant", Theme::dim()),
        ]));
        frame.render_widget(summary, chunks[0]);

        // Score table
        let header = Row::new(vec![
            "#", "Variant", "Type", "Build", "Tests", "Quality", "Novelty", "Score",
        ])
        .style(Theme::header());

        let rows: Vec<Row> = self
            .variants
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let row_style = if i == self.selected {
                    Theme::selected()
                } else {
                    Theme::normal()
                };

                let (build, tests, quality, novelty, score) =
                    if let Some(ref br) = v.benchmark_results {
                        let build = br
                            .execution
                            .as_ref()
                            .map(|e| {
                                if e.build_success {
                                    "Pass".to_string()
                                } else {
                                    "Fail".to_string()
                                }
                            })
                            .unwrap_or_else(|| "—".to_string());

                        let tests = br
                            .execution
                            .as_ref()
                            .and_then(|e| e.test_pass_rate)
                            .map(|r| format!("{:.0}%", r * 100.0))
                            .unwrap_or_else(|| "—".to_string());

                        let quality = br
                            .judge
                            .as_ref()
                            .map(|j| format!("{:.1}", j.code_quality))
                            .unwrap_or_else(|| "—".to_string());

                        let novelty = br
                            .judge
                            .as_ref()
                            .map(|j| format!("{:.1}", j.novelty))
                            .unwrap_or_else(|| "—".to_string());

                        let score = br
                            .composite_score
                            .map(|s| format!("{:.1}", s))
                            .unwrap_or_else(|| "—".to_string());

                        (build, tests, quality, novelty, score)
                    } else {
                        (
                            "—".to_string(),
                            "—".to_string(),
                            "—".to_string(),
                            "—".to_string(),
                            "—".to_string(),
                        )
                    };

                let variant_type = if v.is_merge() { "Merge" } else { "Orig" };

                Row::new(vec![
                    format!("{}", i + 1),
                    truncate(&v.display_name, 25),
                    variant_type.to_string(),
                    build,
                    tests,
                    quality,
                    novelty,
                    score,
                ])
                .style(row_style)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Min(15),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(8),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::TOP));

        frame.render_widget(table, chunks[1]);

        // Bar chart for composite scores
        let bars: Vec<Bar> = self
            .variants
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                v.benchmark_results
                    .as_ref()
                    .and_then(|br| br.composite_score)
                    .map(|score| {
                        let label = format!("V{}", i + 1);
                        Bar::default()
                            .value(score as u64)
                            .label(label.into())
                            .style(Style::default().fg(Theme::score_color(score, 100.0)))
                    })
            })
            .collect();

        if !bars.is_empty() {
            let chart = BarChart::default()
                .block(
                    Block::default()
                        .title(" Composite Scores ")
                        .borders(Borders::ALL)
                        .border_style(Theme::dim()),
                )
                .data(BarGroup::default().bars(&bars))
                .bar_width(5)
                .bar_gap(1)
                .max(100);
            frame.render_widget(chart, chunks[2]);
        }

        // Detail panel
        if let Some(variant) = self.variants.get(self.selected) {
            let detail_block = Block::default()
                .title(format!(" {} — Details ", variant.display_name))
                .borders(Borders::ALL)
                .border_style(Theme::dim());

            let mut lines = vec![Line::from(vec![
                Span::styled("Branch: ", Theme::header()),
                Span::styled(&variant.branch_name, Theme::normal()),
            ])];

            if let Some(ref br) = variant.benchmark_results {
                if let Some(ref judge) = br.judge {
                    lines.push(Line::from(vec![
                        Span::styled("Judge: ", Theme::header()),
                        Span::styled(&judge.explanation, Theme::dim()),
                    ]));
                }
                if let Some(ref user) = br.user_rating {
                    lines.push(Line::from(vec![
                        Span::styled("Your rating: ", Theme::header()),
                        Span::styled(
                            format!("{} — {}", "*".repeat(user.stars as usize), user.notes),
                            Theme::normal(),
                        ),
                    ]));
                }
            }

            let detail = Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .block(detail_block);

            frame.render_widget(detail, chunks[3]);
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
