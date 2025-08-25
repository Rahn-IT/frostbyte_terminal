use std::{collections::VecDeque, fmt::Debug, ops::Range};

use iced::{advanced::text, widget::text::Span};
use wezterm_term::{CellAttributes, PhysRowIndex, Underline};

use crate::{
    Style,
    terminal_grid::PreRenderer,
    wezterm::{
        WeztermGrid,
        selection::{SelectionPosition, is_selected},
    },
};

pub struct WeztermPreRenderer<R: text::Renderer> {
    rows: VecDeque<Option<ParagraphRow<R>>>,
    style: Style,
    visible_range: Range<PhysRowIndex>,
}

impl<R: text::Renderer> WeztermPreRenderer<R> {
    pub(crate) fn new(style: Style) -> Self {
        Self {
            rows: VecDeque::new(),
            style,
            visible_range: 0..0,
        }
    }
}

impl<R> PreRenderer<R> for WeztermPreRenderer<R>
where
    R: text::Renderer,
    R::Font: From<iced::Font>,
    R::Font: 'static,
{
    type Grid = WeztermGrid;

    fn update(&mut self, grid: &Self::Grid, renderer: &R) {
        let prerender = iced::debug::time("prerender");
        let screen = grid.terminal.screen();

        let range = grid.scroll_offset
            ..screen
                .scrollback_rows()
                .min(grid.scroll_offset + screen.physical_rows);
        self.visible_range = range.clone();

        let selection = grid.selection.get_selection();

        let text_size = self
            .style
            .text_size
            .unwrap_or_else(|| renderer.default_size());

        let font: R::Font = self.style.font.into();

        let line_span = iced::debug::time("lines");
        screen.with_phys_lines(range.clone(), |lines| {
            for (offset, line) in lines.iter().enumerate() {
                let mut is_current_selected = false;
                let prepare = iced::debug::time("prepare");
                let index = range.start + offset;

                while self.rows.len() <= index {
                    self.rows.push_back(None);
                }

                if let Some(row) = self.rows.get_mut(index as usize) {
                    if let Some(row) = row {
                        if line.current_seqno() <= row.last_update_seqno {
                            continue;
                        }
                    }
                }

                prepare.finish();
                let span = iced::debug::time("span creation");
                let mut current_text = String::new();
                let mut current_attrs = CellAttributes::default();
                let mut spans: Vec<Span<(), R::Font>> = Vec::new();
                let mut needs_advanced = false;

                for (cell_index, cell) in line.visible_cells().enumerate() {
                    let cell_selected = is_selected(
                        &selection,
                        SelectionPosition {
                            x: cell_index,
                            y: index,
                        },
                    );
                    if cell.attrs() != &current_attrs || is_current_selected != cell_selected {
                        push_span(
                            &self.style,
                            &mut spans,
                            current_text,
                            current_attrs,
                            is_current_selected,
                        );
                        current_attrs = cell.attrs().clone();
                        is_current_selected = cell_selected;
                        current_text = String::new();
                    }
                    let cell_str = cell.str();
                    if !cell_str.is_ascii() {
                        needs_advanced = true;
                    }
                    current_text.push_str(cell_str);
                }

                push_span(
                    &self.style,
                    &mut spans,
                    current_text,
                    current_attrs,
                    is_current_selected,
                );
                span.finish();

                let span = iced::debug::time("text layout");
                let shaping = if needs_advanced {
                    iced::widget::text::Shaping::Advanced
                } else {
                    iced::widget::text::Shaping::Basic
                };
                let row = if !spans.is_empty() {
                    let text = iced::advanced::Text {
                        content: spans.as_slice(),
                        bounds: iced::Size::INFINITE,
                        size: text_size,
                        line_height: iced::advanced::text::LineHeight::default(),
                        font: font,
                        align_x: iced::advanced::text::Alignment::Left,
                        align_y: iced::alignment::Vertical::Top,
                        shaping,
                        wrapping: iced::widget::text::Wrapping::None,
                    };
                    let paragraph = iced::advanced::text::Paragraph::with_spans(text);
                    Some(ParagraphRow {
                        paragraph,
                        spans,
                        last_update_seqno: line.current_seqno(),
                    })
                } else {
                    None
                };
                span.finish();
                *self.rows.get_mut(index).unwrap() = row;
            }
        });
        line_span.finish();
        prerender.finish();
    }

    fn visible_rows<'a>(
        &'a self,
    ) -> impl Iterator<Item = Option<(&'a R::Paragraph, &'a [text::Span<'a, (), R::Font>])>> {
        self.rows.range(self.visible_range.clone()).map(|row| {
            row.as_ref()
                .map(|row| (&row.paragraph, row.spans.as_slice()))
        })
    }
}

fn push_span<Font>(
    style: &Style,
    spans: &mut Vec<Span<(), Font>>,
    text: String,
    attributes: CellAttributes,
    is_current_selected: bool,
) {
    if text.is_empty() {
        return;
    }

    let mut background = style.get_color(attributes.background());
    let mut foreground = style.get_color(attributes.foreground());

    // Apply reverse colors for original cell attributes
    if attributes.reverse() != is_current_selected {
        (background, foreground) = (foreground, background);
        if foreground.is_none() {
            foreground = Some(style.background_color)
        }
        if background.is_none() {
            background = Some(style.foreground_color)
        }
    }

    let span = iced::advanced::text::Span::new(text)
        .color_maybe(foreground)
        .background_maybe(background)
        .underline(attributes.underline() != Underline::None);

    spans.push(span);
}

pub struct ParagraphRow<R: text::Renderer> {
    pub paragraph: R::Paragraph,
    pub spans: Vec<iced::advanced::text::Span<'static, (), R::Font>>,
    pub last_update_seqno: usize,
}

impl<R: text::Renderer> Debug for ParagraphRow<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParagraphRow")
            .field("spans count", &self.spans.len())
            .field("last_update_seqno", &self.last_update_seqno)
            .finish()
    }
}
