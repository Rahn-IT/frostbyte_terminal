use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use iced::advanced::text::Paragraph;
use iced::mouse::ScrollDelta;
#[cfg(feature = "iced-013")]
use iced_013::{self as iced};
use iced_master::Pixels;
#[cfg(feature = "iced-master")]
use iced_master::{self as iced};

use termwiz::surface::{CursorShape, CursorVisibility, SequenceNo};
use tokio::sync::mpsc;
use wezterm_term::{
    CellAttributes, CursorPosition, TerminalConfiguration, Underline,
    color::{ColorAttribute, ColorPalette},
};

pub use wezterm_term::TerminalSize;

#[derive(Debug, Clone, Copy, PartialEq)]
struct GridPosition {
    pub x: usize,
    pub y: usize,
}

impl GridPosition {
    fn into_selection_position(self, horizontal_offset: usize) -> SelectionPosition {
        SelectionPosition {
            x: self.x,
            y: self.y + horizontal_offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SelectionPosition {
    pub x: usize,
    pub y: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum SelectionState {
    None,
    Selecting {
        start: SelectionPosition,
        current: SelectionPosition,
    },
    Selected {
        start: SelectionPosition,
        end: SelectionPosition,
    },
}

impl SelectionState {
    fn start(&mut self, position: SelectionPosition) {
        *self = Self::Selecting {
            start: position.clone(),
            current: position,
        };
    }

    fn move_mouse(&mut self, position: SelectionPosition) {
        match self {
            Self::Selecting { current, .. } => {
                *current = position;
            }
            Self::None => (),
            Self::Selected { .. } => (),
        }
    }

    fn stop(&mut self) {
        match self {
            Self::Selecting { start, current } => {
                if start != current {
                    *self = Self::Selected {
                        start: start.clone(),
                        end: current.clone(),
                    }
                } else {
                    *self = Self::None
                }
            }
            Self::None => (),
            Self::Selected { .. } => (),
        };
    }

    fn is_position_selected(&self, pos: SelectionPosition) -> bool {
        let (start_pos, end_pos) = match self {
            SelectionState::Selecting { start, current } => (start.clone(), current.clone()),
            SelectionState::Selected { start, end } => (start.clone(), end.clone()),
            SelectionState::None => return false,
        };

        // Normalize selection so start is always before end
        let (start_pos, end_pos) =
            if start_pos.y < end_pos.y || (start_pos.y == end_pos.y && start_pos.x <= end_pos.x) {
                (start_pos, end_pos)
            } else {
                (end_pos, start_pos)
            };

        // Check if position is within selection
        if pos.y < start_pos.y || pos.y > end_pos.y {
            return false;
        }

        if pos.y == start_pos.y && pos.y == end_pos.y {
            // Selection is on single line
            pos.x >= start_pos.x && pos.x <= end_pos.x
        } else if pos.y == start_pos.y {
            // First line of multi-line selection
            pos.x >= start_pos.x
        } else if pos.y == end_pos.y {
            // Last line of multi-line selection
            pos.x <= end_pos.x
        } else {
            // Middle line of multi-line selection
            true
        }
    }
}

#[derive(Debug, Clone)]
pub struct MessageWrapper(Message);

#[derive(Debug, Clone)]
enum Message {
    Resize(TerminalSize),
    KeyPress {
        modified_key: iced::keyboard::key::Key,
        modifiers: iced::keyboard::Modifiers,
    },
    Input(Vec<u8>),
    Paste(Option<String>),
    Scrolled(ScrollDelta),
    StartSelection(GridPosition),
    MoveSelection(GridPosition),
    EndSelection,
    ShowContextMenu(iced::Point),
    HideContextMenu,
    ContextMenuCopy,
    ContextMenuPaste,
}

pub enum Action {
    None,
    Run(iced::Task<MessageWrapper>),
    Resize(TerminalSize),
    Input(Vec<u8>),
}

pub struct Terminal {
    term: wezterm_term::Terminal,
    selection_state: SelectionState,
    formatted_rows: VecDeque<FormattedRow>,
    first_row_scrollback_pos: usize,
    last_span_update: SequenceNo,
    last_span_update_scroll_pos: usize,
    id: Option<Id>,
    key_filter: Option<Box<dyn Fn(&iced::keyboard::Key, &iced::keyboard::Modifiers) -> bool>>,
    // here to abort the task on drop
    _handle: iced::task::Handle,
    font: iced::Font,
    scroll_pos: usize,
    padding: iced::Padding,
    background_color: iced::Color,
    foreground_color: iced::Color,
    cursor_pos: CursorPosition,
    context_menu_position: Option<iced::Point>,
    line_height: iced::widget::text::LineHeight,
    text_size: Option<Pixels>,
}

struct FormattedRow {
    last_update_seqno: usize,
    spans: Vec<iced::advanced::text::Span<'static, (), iced::Font>>,
}

impl Default for FormattedRow {
    fn default() -> Self {
        FormattedRow {
            last_update_seqno: 0,
            spans: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Config {}

impl TerminalConfiguration for Config {
    fn color_palette(&self) -> wezterm_term::color::ColorPalette {
        ColorPalette::default()
    }
}

pub struct BridgedWriter {
    send: mpsc::Sender<Vec<u8>>,
}

impl std::io::Write for BridgedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.send.blocking_send(buf.to_vec()).is_ok() {
            Ok(buf.len())
        } else {
            Ok(0)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Terminal {
    pub fn new(rows: u16, cols: u16) -> (Self, iced::Task<MessageWrapper>) {
        let size = TerminalSize {
            rows: rows as usize,
            cols: cols as usize,
            ..Default::default()
        };

        let config = Config {};

        let (send, recv) = mpsc::channel(100);
        let recv = tokio_stream::wrappers::ReceiverStream::new(recv);
        let writer = BridgedWriter { send };

        let term = wezterm_term::Terminal::new(
            size,
            Arc::new(config),
            "frozen_term",
            "0.1",
            Box::new(writer),
        );

        let palette = term.palette();
        let (r, g, b, a) = palette.background.to_tuple_rgba();
        let background_color = iced::Color::from_rgba(r, g, b, a);
        let (r, g, b, a) = palette.foreground.to_tuple_rgba();
        let foreground_color = iced::Color::from_rgba(r, g, b, a);
        let cursor_pos = term.cursor_pos();

        let (task, handle) = iced::Task::run(recv, Message::Input)
            .map(MessageWrapper)
            .abortable();

        let handle = handle.abort_on_drop();

        (
            Self {
                term,
                selection_state: SelectionState::None,
                formatted_rows: VecDeque::new(),
                first_row_scrollback_pos: 0,
                last_span_update: 0,
                last_span_update_scroll_pos: 0,
                id: None,
                _handle: handle,
                key_filter: None,
                font: iced::Font::MONOSPACE,
                scroll_pos: 0,
                padding: 10.into(),
                background_color,
                foreground_color,
                cursor_pos,
                context_menu_position: None,
                line_height: iced::widget::text::LineHeight::default(),
                text_size: None,
            },
            task,
        )
    }

    pub fn id(mut self, id: impl Into<Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn random_id(self) -> Self {
        self.id(Id(iced::advanced::widget::Id::unique()))
    }

    pub fn padding(mut self, padding: impl Into<iced::Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Allows you to add a filter to stop the terminal from capturing keypresses you want to use for your application.
    /// If the given filter returns `true`, the keypress will be ignored.
    pub fn key_filter(
        mut self,
        key_filter: impl 'static + Fn(&iced::keyboard::Key, &iced::keyboard::Modifiers) -> bool,
    ) -> Self {
        self.key_filter = Some(Box::new(key_filter));
        self
    }

    pub fn font(mut self, font: impl Into<iced::Font>) -> Self {
        self.font = font.into();
        self
    }

    pub fn focus<T>(&self) -> iced::Task<T>
    where
        T: Send + 'static,
    {
        if let Some(id) = &self.id {
            Self::focus_with_id(id.clone())
        } else {
            iced::Task::none()
        }
    }

    pub fn focus_with_id<T>(id: Id) -> iced::Task<T>
    where
        T: Send + 'static,
    {
        iced::advanced::widget::operate(iced::advanced::widget::operation::focusable::focus(id.0))
    }

    pub fn get_title(&self) -> &str {
        self.term.get_title()
    }

    pub fn advance_bytes<B>(&mut self, bytes: B)
    where
        B: AsRef<[u8]>,
    {
        self.term.advance_bytes(bytes);
        self.update_formatted_rows(false);
    }

    fn get_selected_text(&self) -> Option<String> {
        let (start_pos, mut end_pos) = match &self.selection_state {
            SelectionState::Selected { start, end } => (start.clone(), end.clone()),
            SelectionState::Selecting { start, current } => (start.clone(), current.clone()),
            SelectionState::None => return None,
        };

        end_pos.x += 1;

        // Normalize selection so start is always before end
        let (start_pos, end_pos) =
            if start_pos.y < end_pos.y || (start_pos.y == end_pos.y && start_pos.x <= end_pos.x) {
                (start_pos, end_pos)
            } else {
                (end_pos, start_pos)
            };

        let screen = self.term.screen();
        let mut selected_text = String::new();

        // Get lines directly from the selection range
        let range = start_pos.y..end_pos.y + 1;
        let term_lines = screen.lines_in_phys_range(range);

        for (line_idx, line) in term_lines.iter().enumerate() {
            let line_y = start_pos.y + line_idx;
            let line_str = line.as_str();

            if line_y == start_pos.y && line_y == end_pos.y {
                // Single line selection
                let start_x = start_pos.x.min(line_str.len());
                let end_x = end_pos.x.min(line_str.len());
                if start_x <= end_x {
                    selected_text.push_str(&line_str[start_x..end_x]);
                }
            } else if line_y == start_pos.y {
                // First line of multi-line selection
                let start_x = start_pos.x.min(line_str.len());
                selected_text.push_str(&line_str[start_x..]);
                selected_text.push('\n');
            } else if line_y == end_pos.y {
                // Last line of multi-line selection
                let end_x = (end_pos.x).min(line_str.len());
                selected_text.push_str(&line_str[..end_x]);
            } else {
                // Middle line of multi-line selection
                selected_text.push_str(&line_str);
                selected_text.push('\n');
            }
        }

        Some(selected_text)
    }

    #[must_use]
    pub fn update(&mut self, message: MessageWrapper) -> Action {
        match message.0 {
            Message::Resize(size) => {
                self.term.resize(size.clone());
                Action::Resize(size)
            }
            Message::KeyPress {
                modified_key,
                modifiers,
            } => {
                if modified_key == iced::keyboard::Key::Character("V".into())
                    && modifiers.control()
                    && modifiers.shift()
                {
                    return self.paste();
                }

                if modified_key == iced::keyboard::Key::Character("C".into())
                    && modifiers.control()
                    && modifiers.shift()
                {
                    return self.copy();
                }

                if let Some((key, modifiers)) = transform_key(modified_key, modifiers) {
                    self.term.key_down(key, modifiers).unwrap();
                    self.scroll_pos = 0;
                    self.update_formatted_rows(false);
                }

                Action::None
            }
            Message::Input(input) => Action::Input(input),
            Message::Paste(paste) => {
                if let Some(paste) = paste {
                    self.term.send_paste(&paste).unwrap();
                    self.scroll_pos = 0;
                    self.update_formatted_rows(false);
                }
                Action::None
            }
            Message::Scrolled(scrolled) => {
                match scrolled {
                    ScrollDelta::Lines { y, .. } => {
                        if y >= 0.0 {
                            self.scroll_pos += y as usize
                        } else {
                            self.scroll_pos = self.scroll_pos.saturating_sub(-y as usize);
                        }
                    }
                    ScrollDelta::Pixels { y, .. } => {
                        if y >= 0.0 {
                            self.scroll_pos += y as usize
                        } else {
                            self.scroll_pos = self.scroll_pos.saturating_sub(-y as usize);
                        }
                    }
                };

                let force_redraw = match self.selection_state {
                    SelectionState::None => false,
                    SelectionState::Selecting { .. } => true,
                    SelectionState::Selected { .. } => false,
                };

                self.update_formatted_rows(force_redraw);

                Action::None
            }
            Message::StartSelection(position) => {
                self.selection_state.start(self.pos_conversion(position));
                self.update_formatted_rows(true);
                Action::None
            }
            Message::MoveSelection(position) => {
                self.selection_state
                    .move_mouse(self.pos_conversion(position));
                self.update_formatted_rows(true);

                Action::None
            }
            Message::EndSelection => {
                self.selection_state.stop();
                self.update_formatted_rows(true);
                Action::None
            }
            Message::ShowContextMenu(position) => {
                self.context_menu_position = Some(position);
                Action::None
            }
            Message::HideContextMenu => {
                self.context_menu_position = None;
                Action::None
            }
            Message::ContextMenuCopy => {
                self.context_menu_position = None;
                self.copy()
            }
            Message::ContextMenuPaste => {
                self.context_menu_position = None;
                self.paste()
            }
        }
    }

    fn copy(&self) -> Action {
        if let Some(selected_text) = self.get_selected_text() {
            return Action::Run(iced::Task::batch([
                iced::clipboard::write(selected_text),
                self.focus(),
            ]));
        }
        Action::None
    }

    fn paste(&self) -> Action {
        Action::Run(iced::Task::batch([
            iced::clipboard::read()
                .map(Message::Paste)
                .map(MessageWrapper),
            self.focus(),
        ]))
    }

    fn pos_conversion(&self, position: GridPosition) -> SelectionPosition {
        let horizontal_offset =
            self.term.screen().scrollback_rows() - self.term.get_size().rows - self.scroll_pos;
        position.into_selection_position(horizontal_offset)
    }

    // Helper to update the current iced text representation from the current wezterm data
    fn update_formatted_rows(&mut self, force: bool) {
        if force {
            self.term.increment_seqno();
        }

        let screen = self.term.screen();
        let max_scroll = screen
            .scrollback_rows()
            .saturating_sub(self.term.get_size().rows);

        // This usually only happens on a `clear` command
        if self.scroll_pos > max_scroll {
            self.scroll_pos = max_scroll;
        }

        if self.needs_row_update() || force {
            self.last_span_update = self.term.current_seqno();
            self.last_span_update_scroll_pos = self.scroll_pos;

            let visible_rows_end = screen.scrollback_rows().saturating_sub(self.scroll_pos);
            let visible_rows_start = visible_rows_end.saturating_sub(screen.physical_rows);
            let visible_range = visible_rows_start..visible_rows_end;

            let term_lines = screen.lines_in_phys_range(visible_range);

            let palette = self.term.palette();

            self.cursor_pos = self.term.cursor_pos();

            let mut is_current_selected = false;
            let range_start = visible_rows_end.saturating_sub(screen.physical_rows);
            let mut absolute_line = range_start;

            // Remove or add rows at the top
            if self.first_row_scrollback_pos > range_start {
                for _ in range_start..self.first_row_scrollback_pos {
                    self.formatted_rows.push_front(FormattedRow::default());
                }
            } else if self.first_row_scrollback_pos < range_start {
                for _ in self.first_row_scrollback_pos..range_start {
                    self.formatted_rows.pop_front();
                }
            }
            self.first_row_scrollback_pos = range_start;

            // Remove or add rows at the bottom
            while self.formatted_rows.len() < screen.physical_rows {
                self.formatted_rows.push_back(FormattedRow::default());
            }
            while self.formatted_rows.len() > screen.physical_rows {
                self.formatted_rows.pop_back();
            }

            for (line, formatted_row) in term_lines.iter().zip(self.formatted_rows.iter_mut()) {
                let mut current_col_idx = 0;

                let mut current_text = String::new();
                let mut current_attrs = CellAttributes::default();

                if line.current_seqno() != formatted_row.last_update_seqno || force {
                    formatted_row.last_update_seqno = self.term.current_seqno();
                    formatted_row.spans.clear();

                    for cell in line.visible_cells() {
                        let char_pos = SelectionPosition {
                            x: current_col_idx,
                            y: absolute_line,
                        };

                        let is_selected = self.selection_state.is_position_selected(char_pos);

                        // Check if we need to break the span due to attribute changes or selection changes
                        if cell.attrs() != &current_attrs || is_selected != is_current_selected {
                            Self::push_span(
                                &mut formatted_row.spans,
                                current_text,
                                current_attrs,
                                &palette,
                                is_current_selected,
                                &self.foreground_color,
                                &self.background_color,
                            );
                            current_attrs = cell.attrs().clone();
                            is_current_selected = is_selected;
                            current_text = String::new();
                        }

                        current_text.push_str(cell.str());
                        current_col_idx += 1;
                    }

                    Self::push_span(
                        &mut formatted_row.spans,
                        current_text,
                        current_attrs,
                        &palette,
                        is_current_selected,
                        &self.foreground_color,
                        &self.background_color,
                    );

                    absolute_line += 1;
                }
            }
        }
    }

    fn needs_row_update(&self) -> bool {
        self.last_span_update != self.term.current_seqno()
            || self.scroll_pos != self.last_span_update_scroll_pos
    }

    pub fn view<'a, Theme, Renderer>(&'a self) -> iced::Element<'a, MessageWrapper, Theme, Renderer>
    where
        Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'static,
        Theme: iced::widget::text::Catalog + 'static,
        Theme: iced::widget::container::Catalog + iced::widget::button::Catalog,
        <Theme as iced::widget::text::Catalog>::Class<'static>:
            From<iced::widget::text::StyleFn<'static, Theme>>,
        <Theme as iced::widget::container::Catalog>::Class<'static>:
            From<iced::widget::container::StyleFn<'static, Theme>>,
    {
        let terminal_widget =
            iced::Element::new(TerminalWidget::new(self, self.font).id_maybe(self.id.clone()));

        if let Some(position) = self.context_menu_position {
            let copy_button = iced::widget::button(iced::widget::text("Copy").size(14))
                .padding([4, 8])
                .width(iced::Length::Fill)
                .on_press(Message::ContextMenuCopy);

            let paste_button = iced::widget::button(iced::widget::text("Paste").size(14))
                .padding([4, 8])
                .width(iced::Length::Fill)
                .on_press(Message::ContextMenuPaste);

            let context_menu = iced::widget::column![copy_button, paste_button].spacing(2);

            let positioned_menu = iced::widget::container(context_menu)
                .style(|_theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgb(
                        0.2, 0.2, 0.2,
                    ))),
                    border: iced::Border {
                        color: iced::Color::from_rgb(0.5, 0.5, 0.5),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .padding(4)
                .width(100)
                .height(iced::Length::Shrink);

            // Position the menu using padding to offset it to the cursor position
            let positioned_container = iced::widget::container(positioned_menu)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .padding(iced::Padding {
                    top: position.y,
                    right: 0.0,
                    bottom: 0.0,
                    left: position.x,
                });

            iced::widget::stack![terminal_widget, positioned_container].into()
        } else {
            terminal_widget
        }
        .map(MessageWrapper)
    }

    fn push_span(
        formatted_row: &mut Vec<iced::advanced::text::Span<'static, (), iced::Font>>,
        current_text: String,
        attributes: CellAttributes,
        palette: &ColorPalette,
        is_selected: bool,
        foreground_color: &iced::Color,
        background_color: &iced::Color,
    ) {
        let mut background =
            get_color(attributes.background(), palette).unwrap_or_else(|| background_color.clone());
        let mut foreground =
            get_color(attributes.foreground(), palette).unwrap_or_else(|| foreground_color.clone());

        if !current_text.is_empty() {
            // Apply reverse colors for original cell attributes
            if attributes.reverse() != is_selected {
                (background, foreground) = (foreground, background);
            }

            let span = iced::advanced::text::Span::new(current_text)
                .color(foreground)
                .background(background)
                .underline(attributes.underline() != Underline::None);

            formatted_row.push(span);
        }
    }
}

fn transform_key(
    key: iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
) -> Option<(wezterm_term::KeyCode, wezterm_term::KeyModifiers)> {
    let wez_key = match key {
        iced::keyboard::Key::Character(c) => {
            let c = c.chars().next().unwrap();
            Some(wezterm_term::KeyCode::Char(c))
        }
        iced::keyboard::Key::Named(named) => match named {
            iced::keyboard::key::Named::Enter => Some(wezterm_term::KeyCode::Enter),
            iced::keyboard::key::Named::Space => Some(wezterm_term::KeyCode::Char(' ')),
            iced::keyboard::key::Named::Backspace => Some(wezterm_term::KeyCode::Backspace),
            iced::keyboard::key::Named::Delete => Some(wezterm_term::KeyCode::Delete),
            iced::keyboard::key::Named::ArrowLeft => Some(wezterm_term::KeyCode::LeftArrow),
            iced::keyboard::key::Named::ArrowRight => Some(wezterm_term::KeyCode::RightArrow),
            iced::keyboard::key::Named::ArrowUp => Some(wezterm_term::KeyCode::UpArrow),
            iced::keyboard::key::Named::ArrowDown => Some(wezterm_term::KeyCode::DownArrow),
            iced::keyboard::key::Named::Tab => Some(wezterm_term::KeyCode::Tab),
            iced::keyboard::key::Named::Escape => Some(wezterm_term::KeyCode::Escape),
            iced::keyboard::key::Named::Home => Some(wezterm_term::KeyCode::Home),
            iced::keyboard::key::Named::End => Some(wezterm_term::KeyCode::End),
            iced::keyboard::key::Named::PageUp => Some(wezterm_term::KeyCode::PageUp),
            iced::keyboard::key::Named::PageDown => Some(wezterm_term::KeyCode::PageDown),
            iced::keyboard::key::Named::F1 => Some(wezterm_term::KeyCode::Function(1)),
            iced::keyboard::key::Named::F2 => Some(wezterm_term::KeyCode::Function(2)),
            iced::keyboard::key::Named::F3 => Some(wezterm_term::KeyCode::Function(3)),
            iced::keyboard::key::Named::F4 => Some(wezterm_term::KeyCode::Function(4)),
            iced::keyboard::key::Named::F5 => Some(wezterm_term::KeyCode::Function(5)),
            iced::keyboard::key::Named::F6 => Some(wezterm_term::KeyCode::Function(6)),
            iced::keyboard::key::Named::F7 => Some(wezterm_term::KeyCode::Function(7)),
            iced::keyboard::key::Named::F8 => Some(wezterm_term::KeyCode::Function(8)),
            iced::keyboard::key::Named::F9 => Some(wezterm_term::KeyCode::Function(9)),
            iced::keyboard::key::Named::F10 => Some(wezterm_term::KeyCode::Function(10)),
            iced::keyboard::key::Named::F11 => Some(wezterm_term::KeyCode::Function(11)),
            iced::keyboard::key::Named::F12 => Some(wezterm_term::KeyCode::Function(12)),
            iced::keyboard::key::Named::F13 => Some(wezterm_term::KeyCode::Function(13)),
            iced::keyboard::key::Named::F14 => Some(wezterm_term::KeyCode::Function(14)),
            iced::keyboard::key::Named::F15 => Some(wezterm_term::KeyCode::Function(15)),
            iced::keyboard::key::Named::F16 => Some(wezterm_term::KeyCode::Function(16)),
            iced::keyboard::key::Named::F17 => Some(wezterm_term::KeyCode::Function(17)),
            iced::keyboard::key::Named::F18 => Some(wezterm_term::KeyCode::Function(18)),
            iced::keyboard::key::Named::F19 => Some(wezterm_term::KeyCode::Function(19)),
            iced::keyboard::key::Named::F20 => Some(wezterm_term::KeyCode::Function(20)),
            iced::keyboard::key::Named::F21 => Some(wezterm_term::KeyCode::Function(21)),
            iced::keyboard::key::Named::F22 => Some(wezterm_term::KeyCode::Function(22)),
            iced::keyboard::key::Named::F23 => Some(wezterm_term::KeyCode::Function(23)),
            iced::keyboard::key::Named::F24 => Some(wezterm_term::KeyCode::Function(24)),
            iced::keyboard::key::Named::F25 => Some(wezterm_term::KeyCode::Function(25)),
            iced::keyboard::key::Named::F26 => Some(wezterm_term::KeyCode::Function(26)),
            iced::keyboard::key::Named::F27 => Some(wezterm_term::KeyCode::Function(27)),
            iced::keyboard::key::Named::F28 => Some(wezterm_term::KeyCode::Function(28)),
            iced::keyboard::key::Named::F29 => Some(wezterm_term::KeyCode::Function(29)),
            iced::keyboard::key::Named::F30 => Some(wezterm_term::KeyCode::Function(30)),
            iced::keyboard::key::Named::F31 => Some(wezterm_term::KeyCode::Function(31)),
            iced::keyboard::key::Named::F32 => Some(wezterm_term::KeyCode::Function(32)),
            iced::keyboard::key::Named::F33 => Some(wezterm_term::KeyCode::Function(33)),
            iced::keyboard::key::Named::F34 => Some(wezterm_term::KeyCode::Function(34)),
            iced::keyboard::key::Named::F35 => Some(wezterm_term::KeyCode::Function(35)),
            _ => None,
        },
        _ => None,
    };

    match wez_key {
        None => None,
        Some(key) => {
            let mut wez_modifiers = wezterm_term::KeyModifiers::empty();

            if modifiers.shift() {
                wez_modifiers |= wezterm_term::KeyModifiers::SHIFT;
            }
            if modifiers.alt() {
                wez_modifiers |= wezterm_term::KeyModifiers::ALT;
            }
            if modifiers.control() {
                wez_modifiers |= wezterm_term::KeyModifiers::CTRL;
            }
            if modifiers.logo() {
                wez_modifiers |= wezterm_term::KeyModifiers::SUPER;
            }

            Some((key, wez_modifiers))
        }
    }
}

fn get_color(color: ColorAttribute, palette: &ColorPalette) -> Option<iced::Color> {
    match color {
        ColorAttribute::TrueColorWithPaletteFallback(srgba_tuple, _)
        | ColorAttribute::TrueColorWithDefaultFallback(srgba_tuple) => {
            let (r, g, b, a) = srgba_tuple.to_tuple_rgba();
            Some(iced::Color::from_rgba(r, g, b, a))
        }
        ColorAttribute::PaletteIndex(index) => {
            let (r, g, b, a) = palette.colors.0[index as usize].to_tuple_rgba();
            Some(iced::Color::from_rgba(r, g, b, a))
        }
        ColorAttribute::Default => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id(iced::advanced::widget::Id);

impl Id {
    /// Creates a custom [`Id`].
    pub fn new(id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        Self(iced::advanced::widget::Id::new(id))
    }

    /// Creates a unique [`Id`].
    ///
    /// This function produces a different [`Id`] every time it is called.
    pub fn unique() -> Self {
        Self(iced::advanced::widget::Id::unique())
    }
}

impl From<Id> for iced::advanced::widget::Id {
    fn from(id: Id) -> Self {
        id.0
    }
}

impl From<&'static str> for Id {
    fn from(id: &'static str) -> Self {
        Self::new(id)
    }
}

impl From<String> for Id {
    fn from(id: String) -> Self {
        Self::new(id)
    }
}

struct TerminalWidget<'a, R: iced::advanced::text::Renderer> {
    id: Option<Id>,
    term: &'a Terminal,
    font: R::Font,
}

impl<'a, R> TerminalWidget<'a, R>
where
    R: iced::advanced::text::Renderer,
{
    pub fn new(term: &'a Terminal, font: impl Into<R::Font>) -> Self {
        Self {
            id: None,
            term,
            font: font.into(),
        }
    }

    pub fn id_maybe(mut self, id: Option<Id>) -> Self {
        self.id = id;
        self
    }

    fn screen_to_grid_position<Renderer>(
        &self,
        screen_pos: iced::Point,
        layout: iced::advanced::Layout<'_>,
        renderer: &Renderer,
    ) -> Option<GridPosition>
    where
        Renderer: iced::advanced::text::Renderer,
    {
        let padding_offset = iced::Vector::new(self.term.padding.left, self.term.padding.top);
        let translation = layout.position() - iced::Point::ORIGIN + padding_offset;

        // Convert screen position to position relative to terminal content
        let relative_pos = screen_pos - translation;

        // Check if position is within terminal bounds
        if relative_pos.x < 0.0 || relative_pos.y < 0.0 {
            return None;
        }

        // Calculate character dimensions
        let text_size = self
            .term
            .text_size
            .unwrap_or_else(|| renderer.default_size());
        let line_height = self.term.line_height.to_absolute(text_size).0;
        let text_size = text_size.0;
        let char_width = text_size * CHAR_WIDTH;

        // Convert to character coordinates
        let char_x = (relative_pos.x / char_width) as usize;
        let char_y = (relative_pos.y / line_height) as usize;

        // Account for scroll offset - the displayed text is offset by scroll_pos
        let absolute_y = char_y;

        Some(GridPosition {
            x: char_x,
            y: absolute_y,
        })
    }

    fn draw_cursor<Renderer>(
        &self,
        renderer: &mut Renderer,
        state: &State<Renderer>,
        translation: iced::Vector,
    ) where
        Renderer: iced::advanced::text::Renderer,
    {
        let is_cursor_visible = self.term.cursor_pos.visibility == CursorVisibility::Visible
            && ((state.now - state.last_cursor_event).as_millis() < CURSOR_BLINK_INTERVAL_MILLIS
                || ((state.now - state.last_cursor_blink).as_millis()
                    / CURSOR_BLINK_INTERVAL_MILLIS)
                    % 2
                    == 0);

        if !is_cursor_visible {
            return;
        }

        let screen = self.term.term.screen();

        // Calculate the scroll-adjusted cursor position using your custom scroll system
        // The cursor position is absolute, but we need to adjust it by the scroll offset
        let cursor_absolute_y = self.term.cursor_pos.y as i64;
        let scroll_offset = self.term.scroll_pos as i64;
        let visible_cursor_y = cursor_absolute_y + scroll_offset;

        // Check if cursor is within the visible area
        if visible_cursor_y < 0 || visible_cursor_y >= screen.physical_rows as i64 {
            // Cursor is outside the visible area due to scrolling
            return;
        }

        // Calculate character dimensions
        let text_size = self
            .term
            .text_size
            .unwrap_or_else(|| renderer.default_size());

        let line_height = self.term.line_height.to_absolute(text_size).0;
        let text_size = text_size.0;
        let char_width = text_size * CHAR_WIDTH;

        let base_cursor_position = iced::Point::new(
            self.term.cursor_pos.x as f32 * char_width,
            visible_cursor_y as f32 * line_height,
        );

        let padding = 1.0;

        let cursor_bounds = match self.term.cursor_pos.shape {
            CursorShape::BlinkingUnderline
            | CursorShape::SteadyUnderline
            | CursorShape::Default => iced::Rectangle::new(
                base_cursor_position
                    + translation
                    + iced::Vector::new(0.0, renderer.default_size().0 * 1.2),
                iced::Size::new(renderer.default_size().0 * CHAR_WIDTH, 1.0),
            ),
            CursorShape::BlinkingBlock | CursorShape::SteadyBlock => iced::Rectangle::new(
                base_cursor_position + translation + iced::Vector::new(padding, padding),
                iced::Size::new(
                    renderer.default_size().0 * CHAR_WIDTH - padding,
                    renderer.default_size().0 * 1.3 - padding,
                ),
            ),
            CursorShape::BlinkingBar | CursorShape::SteadyBar => iced::Rectangle::new(
                base_cursor_position + translation + iced::Vector::new(padding, padding),
                iced::Size::new(1.0, renderer.default_size().0 * 1.3 - padding),
            ),
        };

        renderer.fill_quad(
            iced::advanced::renderer::Quad {
                bounds: cursor_bounds,
                border: iced::Border::default(),
                ..Default::default()
            },
            iced::Color::WHITE,
        );
    }
}

struct State<R: iced::advanced::text::Renderer> {
    focused: bool,
    first_row_scrollback_pos: usize,
    rows: VecDeque<ParagraphRow<R>>,
    last_cursor_blink: Instant,
    last_cursor_event: Instant,
    now: Instant,
}

struct ParagraphRow<R: iced::advanced::text::Renderer> {
    paragraph: R::Paragraph,
    last_layout_seqno: usize,
}

const CHAR_WIDTH: f32 = 0.6;
const CURSOR_BLINK_INTERVAL_MILLIS: u128 = 250;

impl<Renderer> iced::advanced::widget::operation::Focusable for State<Renderer>
where
    Renderer: iced::advanced::text::Renderer,
{
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn unfocus(&mut self) {
        self.focused = false;
    }
}

impl<Renderer> TerminalWidget<'_, Renderer>
where
    Renderer: iced::advanced::text::Renderer,
    Renderer: 'static,
{
    fn combined_update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) -> iced::advanced::graphics::core::event::Status {
        match event {
            iced::Event::Window(iced::window::Event::RedrawRequested(now)) => {
                let term = &self.term.term;
                let screen = term.screen();

                let widget_width = layout.bounds().width - self.term.padding.horizontal();
                let widget_height = layout.bounds().height - self.term.padding.vertical();
                let line_height = renderer.default_size().0;
                let char_width = line_height * CHAR_WIDTH;

                let target_line_count = (0.77 * widget_height / line_height) as usize;
                let target_col_count = (widget_width / char_width) as usize;

                if screen.physical_rows != target_line_count
                    || screen.physical_cols != target_col_count
                {
                    let size = TerminalSize {
                        rows: target_line_count,
                        cols: target_col_count,
                        pixel_height: widget_height as usize,
                        pixel_width: widget_width as usize,
                        ..Default::default()
                    };
                    shell.publish(Message::Resize(size));
                }

                // handle blinking cursor
                let state = tree.state.downcast_mut::<State<Renderer>>();
                if state.focused {
                    state.now = *now;
                    let millis_until_redraw = CURSOR_BLINK_INTERVAL_MILLIS
                        - (*now - state.last_cursor_blink).as_millis()
                            % CURSOR_BLINK_INTERVAL_MILLIS;

                    #[cfg(feature = "iced-master")]
                    shell.request_redraw_at(iced::window::RedrawRequest::At(
                        *now + Duration::from_millis(millis_until_redraw as u64),
                    ));
                    #[cfg(feature = "iced-013")]
                    shell.request_redraw(iced::window::RedrawRequest::At(
                        *now + Duration::from_millis(millis_until_redraw as u64),
                    ));
                }

                iced::advanced::graphics::core::event::Status::Ignored
            }
            iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                shell.publish(Message::Scrolled(delta.clone()));

                iced::advanced::graphics::core::event::Status::Captured
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button)) => {
                let state = tree.state.downcast_mut::<State<Renderer>>();
                let focused = cursor.position_over(layout.bounds()).is_some();

                state.focused = focused;

                if focused {
                    state.last_cursor_event = Instant::now();

                    // Handle text selection start
                    if *button == iced::mouse::Button::Left {
                        // Hide context menu if visible
                        if self.term.context_menu_position.is_some() {
                            shell.publish(Message::HideContextMenu);
                        }

                        if let Some(cursor_position) = cursor.position() {
                            if let Some(char_pos) =
                                self.screen_to_grid_position(cursor_position, layout, renderer)
                            {
                                shell.publish(Message::StartSelection(char_pos));
                                #[cfg(feature = "iced-master")]
                                shell.request_redraw();
                                #[cfg(feature = "iced-013")]
                                shell.request_redraw(iced::window::RedrawRequest::NextFrame);
                            }
                        }
                    }

                    // Handle right-click for context menu
                    if *button == iced::mouse::Button::Right {
                        if let Some(cursor_position) = cursor.position() {
                            shell.publish(Message::ShowContextMenu(cursor_position));
                            #[cfg(feature = "iced-master")]
                            shell.request_redraw();
                            #[cfg(feature = "iced-013")]
                            shell.request_redraw(iced::window::RedrawRequest::NextFrame);
                        }
                    }

                    iced::advanced::graphics::core::event::Status::Captured
                } else {
                    iced::advanced::graphics::core::event::Status::Ignored
                }
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                if let SelectionState::Selecting { current, .. } = &self.term.selection_state {
                    if let Some(char_pos) =
                        self.screen_to_grid_position(*position, layout, renderer)
                    {
                        let horizontal_offset = self.term.term.screen().scrollback_rows()
                            - self.term.term.get_size().rows
                            - self.term.scroll_pos;
                        if &char_pos.into_selection_position(horizontal_offset) != current {
                            shell.publish(Message::MoveSelection(char_pos));
                        }
                    }
                    iced::advanced::graphics::core::event::Status::Captured
                } else {
                    iced::advanced::graphics::core::event::Status::Ignored
                }
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(button)) => {
                if *button == iced::mouse::Button::Left {
                    if let SelectionState::Selecting { .. } = &self.term.selection_state {
                        shell.publish(Message::EndSelection);
                    }
                    iced::advanced::graphics::core::event::Status::Captured
                } else {
                    iced::advanced::graphics::core::event::Status::Ignored
                }
            }
            iced::Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                let state = tree.state.downcast_mut::<State<Renderer>>();
                let focused = cursor.position_over(layout.bounds()).is_some();

                state.focused = focused;

                if focused {
                    state.last_cursor_event = Instant::now();
                    iced::advanced::graphics::core::event::Status::Captured
                } else {
                    iced::advanced::graphics::core::event::Status::Ignored
                }
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                modified_key,
                modifiers,
                ..
            }) => {
                let state = tree.state.downcast_mut::<State<Renderer>>();

                if state.focused {
                    if let Some(filter) = &self.term.key_filter {
                        if filter(&modified_key, &modifiers) {
                            return iced::advanced::graphics::core::event::Status::Ignored;
                        }
                    }

                    state.last_cursor_event = Instant::now();

                    let message = Message::KeyPress {
                        modified_key: modified_key.clone(),
                        modifiers: modifiers.clone(),
                    };
                    shell.publish(message);

                    iced::advanced::graphics::core::event::Status::Captured
                } else {
                    iced::advanced::graphics::core::event::Status::Ignored
                }
            }
            _ => iced::advanced::graphics::core::event::Status::Ignored,
        }
    }
}

impl<Theme, Renderer> iced::advanced::widget::Widget<Message, Theme, Renderer>
    for TerminalWidget<'_, Renderer>
where
    Renderer: iced::advanced::text::Renderer<Font = iced::Font>,
    Renderer: 'static,
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State<Renderer>>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State::<Renderer> {
            focused: false,
            rows: VecDeque::new(),
            first_row_scrollback_pos: 0,
            last_cursor_blink: Instant::now(),
            last_cursor_event: Instant::now(),
            now: Instant::now(),
        })
    }

    fn size(&self) -> iced::Size<iced::Length> {
        iced::Size::new(iced::Length::Fill, iced::Length::Fill)
    }

    fn operate(
        &self,
        tree: &mut iced::advanced::widget::Tree,
        _layout: iced::advanced::Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let state = tree.state.downcast_mut::<State<Renderer>>();

        #[cfg(feature = "iced-master")]
        operation.focusable(self.id.as_ref().map(|id| &id.0), _layout.bounds(), state);
        #[cfg(feature = "iced-013")]
        operation.focusable(state, self.id.as_ref().map(|id| &id.0));
    }

    fn layout(
        &self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let state = tree.state.downcast_mut::<State<Renderer>>();

        // Add or remove rows from the front
        if state.first_row_scrollback_pos > self.term.first_row_scrollback_pos {
            for _ in self.term.first_row_scrollback_pos..state.first_row_scrollback_pos {
                state.rows.push_front(ParagraphRow {
                    last_layout_seqno: 0,
                    paragraph: Renderer::Paragraph::default(),
                });
            }
        } else if state.first_row_scrollback_pos < self.term.first_row_scrollback_pos {
            for _ in state.first_row_scrollback_pos..self.term.first_row_scrollback_pos {
                state.rows.pop_front();
            }
        }
        state.first_row_scrollback_pos = self.term.first_row_scrollback_pos;

        while state.rows.len() < self.term.formatted_rows.len() {
            state.rows.push_back(ParagraphRow {
                last_layout_seqno: 0,
                paragraph: Renderer::Paragraph::default(),
            });
        }
        while state.rows.len() > self.term.formatted_rows.len() {
            state.rows.pop_back();
        }

        for (paragraph_row, formatted_row) in
            state.rows.iter_mut().zip(self.term.formatted_rows.iter())
        {
            if paragraph_row.last_layout_seqno == formatted_row.last_update_seqno {
                continue;
            }
            paragraph_row.last_layout_seqno = formatted_row.last_update_seqno;

            let text_size = self
                .term
                .text_size
                .unwrap_or_else(|| renderer.default_size());

            let text = iced::advanced::Text {
                content: formatted_row.spans.as_ref(),
                bounds: iced::Size::new(
                    limits.max().width,
                    self.term.line_height.to_absolute(text_size).0,
                ),
                size: text_size,
                line_height: iced::advanced::text::LineHeight::default(),
                font: self.font,
                #[cfg(feature = "iced-master")]
                align_x: iced::advanced::text::Alignment::Left,
                #[cfg(feature = "iced-013")]
                horizontal_alignment: iced::alignment::Horizontal::Left,
                #[cfg(feature = "iced-master")]
                align_y: iced::alignment::Vertical::Top,
                #[cfg(feature = "iced-013")]
                vertical_alignment: iced::alignment::Vertical::Top,
                shaping: iced::widget::text::Shaping::Advanced,
                wrapping: iced::widget::text::Wrapping::None,
            };

            paragraph_row.paragraph = iced::advanced::text::Paragraph::with_spans(text);
        }

        iced::advanced::layout::Node::new(limits.max())
    }

    #[cfg(feature = "iced-master")]
    fn update(
        &mut self,
        state: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        self.combined_update(
            state, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    #[cfg(feature = "iced-013")]
    fn on_event(
        &mut self,
        state: &mut iced::advanced::widget::Tree,
        event: iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) -> iced::event::Status {
        self.combined_update(
            state, &event, layout, cursor, renderer, clipboard, shell, viewport,
        )
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let Some(bounds) = layout.bounds().intersection(viewport) else {
            return;
        };

        let state = tree.state.downcast_ref::<State<Renderer>>();
        let padding_offset = iced::Vector::new(self.term.padding.left, self.term.padding.top);
        let translation = layout.position() - iced::Point::ORIGIN + padding_offset;

        // terminal Background
        renderer.fill_quad(
            iced::advanced::renderer::Quad {
                bounds: layout.bounds(),
                ..Default::default()
            },
            self.term.background_color,
        );
        // drawing text background
        for (row_index, (paragraph_row, formatted_row)) in state
            .rows
            .iter()
            .zip(self.term.formatted_rows.iter())
            .enumerate()
        {
            let paragraph = &paragraph_row.paragraph;
            let y_offset =
                paragraph.line_height().to_absolute(paragraph.size()).0 * row_index as f32;

            for (index, span) in formatted_row.spans.iter().enumerate() {
                if let Some(highlight) = span.highlight {
                    let regions = paragraph.span_bounds(index);

                    for bounds in &regions {
                        let bounds = iced::Rectangle::new(
                            bounds.position()
                                - iced::Vector::new(span.padding.left, span.padding.top),
                            bounds.size()
                                + iced::Size::new(
                                    span.padding.horizontal(),
                                    span.padding.vertical(),
                                ),
                        );

                        renderer.fill_quad(
                            iced::advanced::renderer::Quad {
                                bounds: bounds + translation + iced::Vector::new(0.0, y_offset),
                                border: highlight.border,
                                ..Default::default()
                            },
                            highlight.background,
                        );
                    }
                }
            }

            renderer.fill_paragraph(
                &paragraph,
                bounds.position() + padding_offset + iced::Vector::new(0.0, y_offset),
                self.term.foreground_color,
                bounds,
            );
        }

        self.draw_cursor(renderer, &state, translation);
    }
}
