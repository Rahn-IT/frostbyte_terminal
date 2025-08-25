use std::{collections::VecDeque, fmt::Debug, ops::Range, time::Instant};

use iced::{advanced::text, widget::text::Span};
use wezterm_term::{CellAttributes, PhysRowIndex, Underline};

use crate::{
    Style,
    terminal_grid::PreRenderer,
    wezterm::{
        WeztermGrid,
        selection::{SelectionPosition, is_maybe_selected},
    },
};

pub struct WeztermPreRenderer<R: text::Renderer> {
    row_cache_start: PhysRowIndex,
    max_cache_size: usize,
    cache_rows: VecDeque<ParagraphRow<R>>,
    style: Style,
    visible_cache_range: Range<PhysRowIndex>,
}

impl<R: text::Renderer> WeztermPreRenderer<R> {
    pub(crate) fn new(style: Style) -> Self {
        Self {
            row_cache_start: 0,
            max_cache_size: 100,
            cache_rows: VecDeque::new(),
            style,
            visible_cache_range: 0..0,
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
        let screen = grid.terminal.screen();

        let range = grid.scroll_offset..grid.scroll_offset + screen.physical_rows;

        let selection = grid.selection.get_selection();

        let text_size = self
            .style
            .text_size
            .unwrap_or_else(|| renderer.default_size());

        let font: R::Font = self.style.font.into();

        // Make sure our paragraph cache is ready and aligned

        let row_cache_end = self.row_cache_start + self.cache_rows.len();
        let free_space = self.max_cache_size - self.cache_rows.len();

        if range.end > row_cache_end {
            let missing = range.end - row_cache_end;
            let missing_space = missing.saturating_sub(free_space);
            if missing_space >= self.cache_rows.len() {
                self.cache_rows.clear();
            } else if missing_space > 0 {
                self.cache_rows.drain(0..missing_space);
            }
            self.row_cache_start += missing_space;
            for _ in 0..missing {
                self.cache_rows.push_back(ParagraphRow::default());
            }
        }

        if range.start < self.row_cache_start {
            let missing = self.row_cache_start - range.start;
            let missing_space = missing.saturating_sub(free_space);
            if missing_space >= self.cache_rows.len() {
                self.cache_rows.clear();
            } else if missing_space > 0 {
                self.cache_rows
                    .drain(self.cache_rows.len() - missing_space..self.cache_rows.len());
            }
            for _ in 0..missing {
                self.cache_rows.push_front(ParagraphRow::default());
            }
            self.row_cache_start -= missing_space;
        }

        self.visible_cache_range =
            range.start - self.row_cache_start..range.end - self.row_cache_start;

        for (offset, line) in grid.screen_lines(range.clone()).iter().enumerate() {
            let mut is_current_selected = false;
            let index = range.start + offset;
            let cache_index = index - self.row_cache_start;

            // Checking if row is cached
            if line.current_seqno() <= self.cache_rows[cache_index].last_update_seqno {
                continue;
            }

            let mut current_text = String::new();
            let mut current_attrs = CellAttributes::default();
            let mut spans: Vec<Span<(), R::Font>> = Vec::new();
            let mut needs_advanced = false;

            for (cell_index, cell) in line.visible_cells().enumerate() {
                let cell_selected = is_maybe_selected(
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

            let shaping = if needs_advanced {
                iced::widget::text::Shaping::Advanced
            } else {
                iced::widget::text::Shaping::Basic
            };
            let cached = if !spans.is_empty() {
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
                Some((paragraph, spans))
            } else {
                None
            };
            let row = &mut self.cache_rows[cache_index];
            row.cached = cached;
            row.last_update_seqno = line.current_seqno();
        }
    }

    fn visible_rows<'a>(
        &'a self,
    ) -> impl Iterator<Item = Option<(&'a R::Paragraph, &'a [text::Span<'a, (), R::Font>])>> {
        self.cache_rows
            .range(self.visible_cache_range.clone())
            .map(|row| {
                row.cached
                    .as_ref()
                    .map(|cached| (&cached.0, cached.1.as_slice()))
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
    pub cached: Option<(
        R::Paragraph,
        Vec<iced::advanced::text::Span<'static, (), R::Font>>,
    )>,
    // pub paragraph: R::Paragraph,
    // pub spans: Vec<iced::advanced::text::Span<'static, (), R::Font>>,
    pub last_update_seqno: usize,
}

impl<R: text::Renderer> Default for ParagraphRow<R> {
    fn default() -> Self {
        Self {
            cached: None,
            last_update_seqno: 0,
        }
    }
}

impl<R: text::Renderer> Debug for ParagraphRow<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParagraphRow")
            .field("last_update_seqno", &self.last_update_seqno)
            .finish()
    }
}
