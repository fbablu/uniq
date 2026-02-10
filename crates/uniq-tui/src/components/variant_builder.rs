//! Phase 4: Variant Generation — show progress of generating each variant.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use crate::action::Action;
use crate::components::Component;
use crate::theme::Theme;

use uniq_core::variant::{Variant, VariantStatus};

pub struct VariantBuilderComponent {
    /// All variants (original + merged).
    pub variants: Vec<Variant>,
    /// Currently selected variant.
    pub selected: usize,
    /// Whether generation is in progress.
    pub generating: bool,
}

impl VariantBuilderComponent {
    pub fn new() -> Self {
        Self {
            variants: Vec::new(),
            selected: 0,
            generating: false,
        }
    }

    fn status_display(status: &VariantStatus) -> (String, Style) {
        match status {
            VariantStatus::Pending => ("Pending".to_string(), Theme::dim()),
            VariantStatus::Generating => (
                "Generating...".to_string(),
                Style::default().fg(Theme::warning()),
            ),
            VariantStatus::Ready => ("Ready".to_string(), Style::default().fg(Theme::success())),
            VariantStatus::Failed(err) => (
                format!("Failed: {}", truncate(err, 30)),
                Style::default().fg(Theme::error()),
            ),
        }
    }
}

impl Component for VariantBuilderComponent {
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
            Action::VariantGenerated(variant) => {
                // Update existing or add new.
                if let Some(existing) = self.variants.iter_mut().find(|v| v.id == variant.id) {
                    *existing = *variant.clone();
                } else {
                    self.variants.push(*variant.clone());
                }
                None
            }
            Action::VariantGenerationFailed { variant_id, error } => {
                if let Some(variant) = self.variants.iter_mut().find(|v| v.id.0 == *variant_id) {
                    variant.status = VariantStatus::Failed(error.clone());
                }
                None
            }
            Action::GenerationComplete => {
                self.generating = false;
                let ready = self
                    .variants
                    .iter()
                    .filter(|v| v.status == VariantStatus::Ready)
                    .count();
                Some(Action::SetStatus(format!(
                    "{}/{} variants generated. Press [Right] to benchmark.",
                    ready,
                    self.variants.len()
                )))
            }
            Action::MergeComplete(variant) => {
                self.variants.push(*variant.clone());
                None
            }
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Variant Generation ")
            .title_style(Theme::title())
            .borders(Borders::ALL)
            .border_style(Theme::dim());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.variants.is_empty() {
            let msg = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("No variants yet.", Theme::dim())),
                Line::from(Span::styled(
                    "Select techniques in Phase 3 first.",
                    Theme::dim(),
                )),
            ]);
            frame.render_widget(msg, inner);
            return;
        }

        let chunks = Layout::vertical([
            Constraint::Length(2), // Summary
            Constraint::Min(10),   // Variant table
            Constraint::Length(6), // Detail
        ])
        .split(inner);

        // Summary
        let ready_count = self
            .variants
            .iter()
            .filter(|v| v.status == VariantStatus::Ready)
            .count();
        let summary = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{}/{} variants ready", ready_count, self.variants.len()),
                Theme::header(),
            ),
            Span::styled("  |  ", Theme::dim()),
            Span::styled("[m]", Theme::selected()),
            Span::styled("erge variants  ", Theme::dim()),
            Span::styled("[Right]", Theme::selected()),
            Span::styled(" benchmark", Theme::dim()),
        ]));
        frame.render_widget(summary, chunks[0]);

        // Variant table
        let header = Row::new(vec!["#", "Name", "Type", "Branch", "Status"]).style(Theme::header());

        let rows: Vec<Row> = self
            .variants
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let (status_text, _status_style) = Self::status_display(&v.status);
                let row_style = if i == self.selected {
                    Theme::selected()
                } else {
                    Theme::normal()
                };
                let variant_type = if v.is_merge() { "Merge" } else { "Research" };

                Row::new(vec![
                    format!("{}", i + 1),
                    truncate(&v.display_name, 35),
                    variant_type.to_string(),
                    truncate(&v.branch_name, 30),
                    status_text,
                ])
                .style(row_style)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Min(20),
                Constraint::Length(10),
                Constraint::Length(32),
                Constraint::Length(20),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::TOP));

        frame.render_widget(table, chunks[1]);

        // Detail for selected variant
        if let Some(variant) = self.variants.get(self.selected) {
            let detail_block = Block::default()
                .title(format!(" {} ", variant.display_name))
                .borders(Borders::ALL)
                .border_style(Theme::dim());

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Branch: ", Theme::header()),
                    Span::styled(&variant.branch_name, Theme::normal()),
                ]),
                Line::from(vec![
                    Span::styled("Modified files: ", Theme::header()),
                    Span::styled(format!("{}", variant.modified_files.len()), Theme::normal()),
                ]),
                Line::from(vec![
                    Span::styled("New deps: ", Theme::header()),
                    Span::styled(variant.new_dependencies.join(", "), Theme::normal()),
                ]),
            ];

            if variant.is_merge() {
                lines.push(Line::from(Span::styled(
                    "Type: Merged variant",
                    Style::default().fg(Theme::accent_secondary()),
                )));
            }

            let detail = Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .block(detail_block);

            frame.render_widget(detail, chunks[2]);
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
