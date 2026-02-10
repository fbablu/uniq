//! Merge Dialog — overlay for selecting two variants and a blend ratio.

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::action::Action;
use crate::components::Component;
use crate::theme::Theme;

use uniq_core::merge::BlendRatio;
use uniq_core::variant::Variant;

/// Which field in the merge dialog is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeField {
    VariantA,
    VariantB,
    BlendSlider,
}

pub struct MergeDialogComponent {
    /// Whether the dialog is visible.
    pub visible: bool,
    /// Available variants to merge.
    pub available_variants: Vec<(String, String)>, // (id, display_name)
    /// Selected index for variant A.
    pub variant_a_idx: usize,
    /// Selected index for variant B.
    pub variant_b_idx: usize,
    /// Current blend ratio for A.
    pub blend_a: BlendRatio,
    /// Current blend ratio for B.
    pub blend_b: BlendRatio,
    /// Which field is focused.
    focused: MergeField,
    /// Whether a merge is in progress.
    pub merging: bool,
}

impl MergeDialogComponent {
    pub fn new() -> Self {
        Self {
            visible: false,
            available_variants: Vec::new(),
            variant_a_idx: 0,
            variant_b_idx: 1,
            blend_a: BlendRatio::Half,
            blend_b: BlendRatio::Half,
            focused: MergeField::VariantA,
            merging: false,
        }
    }

    /// Update the available variants list.
    pub fn set_variants(&mut self, variants: &[Variant]) {
        self.available_variants = variants
            .iter()
            .map(|v| (v.id.0.clone(), v.display_name.clone()))
            .collect();
    }

    /// Center a rectangle inside another.
    fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
        let vertical = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .flex(Flex::Center)
        .split(area);

        let horizontal = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .flex(Flex::Center)
        .split(vertical[1]);

        horizontal[1]
    }

    /// Render the blend slider visualization.
    fn render_blend_bar(
        blend_a: &BlendRatio,
        _blend_b: &BlendRatio,
        width: usize,
    ) -> Line<'static> {
        let pct_a = blend_a.as_percent() as usize;
        let filled_a = (width * pct_a) / 100;
        let filled_b = width - filled_a;

        let bar_a = "\u{2588}".repeat(filled_a);
        let bar_b = "\u{2591}".repeat(filled_b);

        Line::from(vec![
            Span::styled(
                "A ",
                Style::default()
                    .fg(Theme::blend_a())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(bar_a, Style::default().fg(Theme::blend_a())),
            Span::styled(bar_b, Style::default().fg(Theme::blend_b())),
            Span::styled(
                " B",
                Style::default()
                    .fg(Theme::blend_b())
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    }
}

impl Component for MergeDialogComponent {
    fn handle_action(&mut self, action: &Action) -> Option<Action> {
        if !self.visible {
            if matches!(action, Action::OpenMergeDialog) {
                self.visible = true;
                self.focused = MergeField::VariantA;
            }
            return None;
        }

        match action {
            Action::CloseMergeDialog => {
                self.visible = false;
                None
            }
            Action::ScrollDown | Action::SelectNext => {
                match self.focused {
                    MergeField::VariantA => self.focused = MergeField::VariantB,
                    MergeField::VariantB => self.focused = MergeField::BlendSlider,
                    MergeField::BlendSlider => {}
                }
                None
            }
            Action::ScrollUp | Action::SelectPrev => {
                match self.focused {
                    MergeField::VariantA => {}
                    MergeField::VariantB => self.focused = MergeField::VariantA,
                    MergeField::BlendSlider => self.focused = MergeField::VariantB,
                }
                None
            }
            Action::NextPhase => {
                // Right arrow: cycle selection or increase blend
                match self.focused {
                    MergeField::VariantA => {
                        if self.variant_a_idx + 1 < self.available_variants.len() {
                            self.variant_a_idx += 1;
                        }
                    }
                    MergeField::VariantB => {
                        if self.variant_b_idx + 1 < self.available_variants.len() {
                            self.variant_b_idx += 1;
                        }
                    }
                    MergeField::BlendSlider => {
                        self.blend_a = self.blend_a.next();
                        self.blend_b = self.blend_a.prev(); // Inverse
                    }
                }
                None
            }
            Action::PrevPhase => {
                // Left arrow
                match self.focused {
                    MergeField::VariantA => {
                        if self.variant_a_idx > 0 {
                            self.variant_a_idx -= 1;
                        }
                    }
                    MergeField::VariantB => {
                        if self.variant_b_idx > 0 {
                            self.variant_b_idx -= 1;
                        }
                    }
                    MergeField::BlendSlider => {
                        self.blend_a = self.blend_a.prev();
                        self.blend_b = self.blend_a.next();
                    }
                }
                None
            }
            Action::Confirm => {
                if self.variant_a_idx == self.variant_b_idx {
                    return Some(Action::SetStatus(
                        "Cannot merge a variant with itself".to_string(),
                    ));
                }
                if self.available_variants.len() < 2 {
                    return Some(Action::SetStatus(
                        "Need at least 2 variants to merge".to_string(),
                    ));
                }

                let va = &self.available_variants[self.variant_a_idx];
                let vb = &self.available_variants[self.variant_b_idx];

                self.merging = true;
                self.visible = false;

                Some(Action::StartMerge {
                    variant_a_id: va.0.clone(),
                    variant_b_id: vb.0.clone(),
                    blend_a: self.blend_a.as_percent(),
                    blend_b: self.blend_b.as_percent(),
                })
            }
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let dialog_area = Self::centered_rect(area, 60, 18);

        // Clear the background.
        frame.render_widget(Clear, dialog_area);

        let block = Block::default()
            .title(" Merge Variants ")
            .title_style(Theme::title())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::accent()));

        let inner = block.inner(dialog_area);
        frame.render_widget(block, dialog_area);

        let chunks = Layout::vertical([
            Constraint::Length(3), // Variant A selector
            Constraint::Length(3), // Variant B selector
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Blend slider
            Constraint::Length(1), // Blend label
            Constraint::Length(1), // Spacer
            Constraint::Length(2), // Instructions
        ])
        .split(inner);

        // Variant A selector
        let a_style = if self.focused == MergeField::VariantA {
            Style::default()
                .fg(Theme::accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Theme::normal()
        };
        let a_name = self
            .available_variants
            .get(self.variant_a_idx)
            .map(|(_, name)| name.as_str())
            .unwrap_or("(none)");
        let va_block = Block::default()
            .title(" Source A ")
            .borders(Borders::ALL)
            .border_style(a_style);
        let va_text =
            Paragraph::new(Span::styled(format!("< {} >", a_name), a_style)).block(va_block);
        frame.render_widget(va_text, chunks[0]);

        // Variant B selector
        let b_style = if self.focused == MergeField::VariantB {
            Style::default()
                .fg(Theme::accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Theme::normal()
        };
        let b_name = self
            .available_variants
            .get(self.variant_b_idx)
            .map(|(_, name)| name.as_str())
            .unwrap_or("(none)");
        let vb_block = Block::default()
            .title(" Source B ")
            .borders(Borders::ALL)
            .border_style(b_style);
        let vb_text =
            Paragraph::new(Span::styled(format!("< {} >", b_name), b_style)).block(vb_block);
        frame.render_widget(vb_text, chunks[1]);

        // Blend slider
        let slider_style = if self.focused == MergeField::BlendSlider {
            Style::default()
                .fg(Theme::accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Theme::normal()
        };
        let bar_width = (chunks[3].width as usize).saturating_sub(6);
        let blend_bar = Self::render_blend_bar(&self.blend_a, &self.blend_b, bar_width);

        let slider_block = Block::default()
            .title(" Integration Blend ")
            .borders(Borders::ALL)
            .border_style(slider_style);
        let slider_para = Paragraph::new(blend_bar).block(slider_block);
        frame.render_widget(slider_para, chunks[3]);

        // Blend label
        let label = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("  A: {} ", self.blend_a),
                Style::default().fg(Theme::blend_a()),
            ),
            Span::styled(" / ", Theme::dim()),
            Span::styled(
                format!(" B: {}", self.blend_b),
                Style::default().fg(Theme::blend_b()),
            ),
        ]));
        frame.render_widget(label, chunks[4]);

        // Instructions
        let instructions = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("[Up/Down]", Theme::selected()),
                Span::styled(" switch field  ", Theme::dim()),
                Span::styled("[Left/Right]", Theme::selected()),
                Span::styled(" adjust  ", Theme::dim()),
                Span::styled("[Enter]", Theme::selected()),
                Span::styled(" merge", Theme::dim()),
            ]),
            Line::from(vec![
                Span::styled("[Esc]", Theme::selected()),
                Span::styled(" cancel", Theme::dim()),
            ]),
        ]);
        frame.render_widget(instructions, chunks[6]);
    }
}
