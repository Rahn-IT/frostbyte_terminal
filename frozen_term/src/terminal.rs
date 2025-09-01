use std::time::{Duration, Instant};

use iced::{
    Rectangle, Size, Vector,
    advanced::{text::Paragraph, widget::operation::Focusable},
    mouse::ScrollDelta,
    widget::{container, row},
};

use crate::{
    Style,
    scrollbar::Scrollbar,
    terminal_grid::{PreRenderer, TerminalGrid, VisiblePosition},
    wezterm::{WeztermGrid, prerenderer::WeztermPreRenderer},
};

pub mod style;
use style::CursorShape;

#[derive(Debug, Clone)]
pub struct Message(InnerMessage);

#[derive(Debug, Clone)]
enum InnerMessage {
    Resize(crate::terminal_grid::Size),
    KeyPress {
        modified_key: iced::keyboard::key::Key,
        modifiers: iced::keyboard::Modifiers,
    },
    Input(Vec<u8>),
    Paste(Option<String>),
    Scrolled(ScrollDelta),
    ScrollTo(usize),
    ScrollDone,
    StartSelection(VisiblePosition),
    MoveSelection(VisiblePosition),
    EndSelection,
    ShowContextMenu(iced::Point),
    HideContextMenu,
    ContextMenuCopy,
    ContextMenuPaste,
    IdChanged,
}

pub enum Action {
    None,
    Run(iced::Task<Message>),
    Resize(crate::terminal_grid::Size),
    Input(Vec<u8>),
    IdChanged,
}

pub struct Terminal {
    grid: WeztermGrid,
    id: Id,
    key_filter: Option<Box<dyn Fn(&iced::keyboard::Key, &iced::keyboard::Modifiers) -> bool>>,
    // here to abort the task on drop
    context_menu_position: Option<iced::Point>,
    style: Style,
    _handle: iced::task::Handle,
}

impl Terminal {
    pub fn new() -> (Self, iced::Task<Message>) {
        let (grid, stream) = WeztermGrid::new();
        let (task, handle) = iced::Task::run(stream, InnerMessage::Input)
            .map(Message)
            .abortable();

        let handle = handle.abort_on_drop();

        (
            Self {
                grid,
                id: Id(iced::advanced::widget::Id::unique()),
                key_filter: None,
                context_menu_position: None,
                style: Style::default(),
                _handle: handle,
            },
            task,
        )
    }

    pub fn id(mut self, id: impl Into<Id>) -> Self {
        self.id = id.into();
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.set_style(style);
        self
    }

    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }

    /// Allows you to add a filter to stop the terminal from capturing keypresses you want to use for your application.
    /// If the given filter returns `true`, the keypress will be ignored.
    pub fn key_filter(
        mut self,
        key_filter: impl 'static + Fn(&iced::keyboard::Key, &iced::keyboard::Modifiers) -> bool,
    ) -> Self {
        self.set_key_filter(key_filter);
        self
    }

    pub fn set_key_filter(
        &mut self,
        key_filter: impl 'static + Fn(&iced::keyboard::Key, &iced::keyboard::Modifiers) -> bool,
    ) {
        self.key_filter = Some(Box::new(key_filter));
    }

    pub fn focus<T>(&self) -> iced::Task<T>
    where
        T: Send + 'static,
    {
        Self::focus_with_id(self.id.clone())
    }

    pub fn focus_with_id<T>(id: impl Into<Id>) -> iced::Task<T>
    where
        T: Send + 'static,
    {
        iced::advanced::widget::operate(iced::advanced::widget::operation::focusable::focus(
            id.into().0,
        ))
    }

    pub fn get_title(&self) -> &str {
        self.grid.get_title()
    }

    pub fn advance_bytes<B>(&mut self, bytes: B)
    where
        B: AsRef<[u8]>,
    {
        self.grid.advance_bytes(bytes.as_ref());
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message.0 {
            InnerMessage::Resize(size) => {
                self.grid.resize(size);
                Action::Resize(size)
            }
            InnerMessage::KeyPress {
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

                if let Some(input) = self.grid.press_key(modified_key, modifiers) {
                    Action::Input(input)
                } else {
                    Action::None
                }
            }
            InnerMessage::Input(input) => Action::Input(input),
            InnerMessage::Paste(paste) => {
                if let Some(paste) = paste {
                    if let Some(input) = self.grid.paste(&paste) {
                        return Action::Input(input);
                    }
                }
                Action::None
            }
            InnerMessage::Scrolled(scrolled) => {
                match scrolled {
                    ScrollDelta::Lines { y, .. } => {
                        self.grid.scroll(y as isize);
                    }
                    ScrollDelta::Pixels { y, .. } => {
                        self.grid.scroll(y as isize);
                    }
                };

                Action::None
            }
            InnerMessage::ScrollTo(y) => {
                self.grid.scroll_to(y);
                Action::None
            }
            InnerMessage::ScrollDone => Action::Run(self.focus()),
            InnerMessage::StartSelection(start) => {
                self.grid.start_selection(start);
                Action::None
            }
            InnerMessage::MoveSelection(position) => {
                self.grid.move_selection(position);
                Action::None
            }
            InnerMessage::EndSelection => {
                self.grid.end_selection();
                Action::None
            }
            InnerMessage::ShowContextMenu(position) => {
                self.context_menu_position = Some(position);
                Action::None
            }
            InnerMessage::HideContextMenu => {
                self.context_menu_position = None;
                Action::None
            }
            InnerMessage::ContextMenuCopy => {
                self.context_menu_position = None;
                self.copy()
            }
            InnerMessage::ContextMenuPaste => {
                self.context_menu_position = None;
                self.paste()
            }
            InnerMessage::IdChanged => Action::IdChanged,
        }
    }

    fn copy(&self) -> Action {
        if let Some(selected_text) = self.grid.selected_text() {
            Action::Run(iced::clipboard::write(selected_text).chain(self.focus()))
        } else {
            Action::Run(self.focus())
        }
    }

    fn paste(&self) -> Action {
        Action::Run(
            iced::clipboard::read()
                .map(InnerMessage::Paste)
                .map(Message)
                .chain(self.focus()),
        )
    }

    pub fn view<'a, Theme, Renderer>(&'a self) -> iced::Element<'a, Message, Theme, Renderer>
    where
        Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'static,
        Theme: iced::widget::text::Catalog + 'static,
        Theme: iced::widget::container::Catalog + iced::widget::button::Catalog,
        <Theme as iced::widget::text::Catalog>::Class<'static>:
            From<iced::widget::text::StyleFn<'static, Theme>>,
        <Theme as iced::widget::container::Catalog>::Class<'static>:
            From<iced::widget::container::StyleFn<'static, Theme>>,
        Theme: iced::widget::scrollable::Catalog,
    {
        self.view_internal().map(Message)
    }

    fn view_internal<'a, Theme, Renderer>(
        &'a self,
    ) -> iced::Element<'a, InnerMessage, Theme, Renderer>
    where
        Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'static,
        Theme: iced::widget::text::Catalog + 'static,
        Theme: iced::widget::container::Catalog + iced::widget::button::Catalog,
        <Theme as iced::widget::text::Catalog>::Class<'static>:
            From<iced::widget::text::StyleFn<'static, Theme>>,
        <Theme as iced::widget::container::Catalog>::Class<'static>:
            From<iced::widget::container::StyleFn<'static, Theme>>,
        Theme: iced::widget::scrollable::Catalog,
    {
        let total_rows = self.grid.available_lines();

        let terminal_widget = row![
            iced::Element::new(TerminalWidget::new(self)),
            Scrollbar::new(
                total_rows as u32,
                self.grid.get_scroll() as u32,
                self.grid.get_size().rows as u32
            )
            .on_scroll(move |scroll| {
                let new_scroll = (total_rows as f32 * scroll) as usize;
                InnerMessage::ScrollTo(new_scroll)
            })
            .on_scroll_done(InnerMessage::ScrollDone)
        ];

        if let Some(position) = self.context_menu_position {
            let copy_button = iced::widget::button(iced::widget::text("Copy").size(14))
                .padding([4, 8])
                .width(iced::Length::Fill)
                .on_press(InnerMessage::ContextMenuCopy);

            let paste_button = iced::widget::button(iced::widget::text("Paste").size(14))
                .padding([4, 8])
                .width(iced::Length::Fill)
                .on_press(InnerMessage::ContextMenuPaste);

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
            let positioned_container = container(positioned_menu)
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
            iced::widget::stack![terminal_widget].into()
        }
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

struct TerminalWidget<'a> {
    term: &'a Terminal,
}

struct State<R: iced::advanced::text::Renderer> {
    prerenderer: WeztermPreRenderer<R>,
    focused: bool,
    last_cursor_blink: Instant,
    cursor_blink_currently_shown: bool,
    now: Instant,
    last_widget_width: f32,
    last_widget_height: f32,
    last_id: Option<Id>,
}

const CHAR_WIDTH: f32 = 0.6;
const CURSOR_BLINK_INTERVAL_MILLIS: u128 = 500;

impl<Renderer> iced::advanced::widget::operation::Focusable for State<Renderer>
where
    Renderer: iced::advanced::text::Renderer,
{
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn focus(&mut self) {
        self.focused = true;
        self.last_cursor_blink = Instant::now();
        self.cursor_blink_currently_shown = true;
    }

    fn unfocus(&mut self) {
        self.focused = false;
        self.cursor_blink_currently_shown = false;
    }
}

impl<Theme, Renderer> iced::advanced::widget::Widget<InnerMessage, Theme, Renderer>
    for TerminalWidget<'_>
where
    Renderer: iced::advanced::text::Renderer<Font = iced::Font>,
    Renderer: 'static,
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State<Renderer>>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State::<Renderer> {
            prerenderer: WeztermPreRenderer::new(self.term.style.clone()),
            focused: false,
            last_cursor_blink: Instant::now(),
            cursor_blink_currently_shown: false,
            now: Instant::now(),
            // needs to be none to detect newly created widgets
            last_id: None,
            last_widget_height: 0.0,
            last_widget_width: 0.0,
        })
    }

    fn size(&self) -> iced::Size<iced::Length> {
        iced::Size::new(iced::Length::Fill, iced::Length::Fill)
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let state = tree.state.downcast_mut::<State<Renderer>>();

        operation.focusable(Some(&self.term.id.0), layout.bounds(), state);
    }

    fn update(
        &mut self,
        state: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, InnerMessage>,
        _viewport: &iced::Rectangle,
    ) {
        match event {
            iced::Event::Window(iced::window::Event::RedrawRequested(now)) => {
                let state = state.state.downcast_mut::<State<Renderer>>();

                let widget_width = layout.bounds().width - self.term.style.padding.horizontal();
                let widget_height = layout.bounds().height - self.term.style.padding.vertical();

                // check if id has changed
                let id_changed = state
                    .last_id
                    .as_ref()
                    .map(|last_id| last_id != &self.term.id)
                    .unwrap_or(true);

                if id_changed {
                    state.last_id = Some(self.term.id.clone());
                    shell.publish(InnerMessage::IdChanged);
                }

                // check if widget size has changed
                if state.last_widget_width != widget_width
                    || state.last_widget_height != widget_height
                    || id_changed
                {
                    state.last_widget_width = widget_width;
                    state.last_widget_height = widget_height;

                    let text_size = self.term.style.text_size.unwrap_or(renderer.default_size());
                    let line_height = self.term.style.line_height.to_absolute(text_size);
                    let char_width = text_size * CHAR_WIDTH;

                    let target_line_count = (widget_height / line_height.0) as usize;
                    let target_col_count = (widget_width / char_width.0) as usize;
                    let size = self.term.grid.get_size();

                    if size.rows != target_line_count || size.cols != target_col_count {
                        let size = crate::terminal_grid::Size {
                            cols: target_col_count,
                            rows: target_line_count,
                        };
                        shell.publish(InnerMessage::Resize(size));
                    }
                }

                // handle blinking cursor
                if state.is_focused() {
                    state.now = *now;
                    let millis_until_redraw = CURSOR_BLINK_INTERVAL_MILLIS
                        .saturating_sub((*now - state.last_cursor_blink).as_millis());

                    if millis_until_redraw == 0 {
                        state.cursor_blink_currently_shown = !state.cursor_blink_currently_shown;
                        state.last_cursor_blink = *now;
                    }

                    shell.request_redraw_at(
                        *now + Duration::from_millis(millis_until_redraw as u64),
                    );
                } else if state.cursor_blink_currently_shown == true {
                    state.cursor_blink_currently_shown = false;
                    shell.request_redraw();
                }
            }
            iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                if cursor.position_over(layout.bounds()).is_some() {
                    shell.publish(InnerMessage::Scrolled(delta.clone()));
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button)) => {
                let state = state.state.downcast_mut::<State<Renderer>>();
                let newly_focused = cursor.position_over(layout.bounds()).is_some();

                if newly_focused {
                    state.focus();

                    // Handle text selection start
                    if *button == iced::mouse::Button::Left {
                        // Hide context menu if visible
                        if self.term.context_menu_position.is_some() {
                            shell.publish(InnerMessage::HideContextMenu);
                        }

                        if let Some(cursor_position) = cursor.position() {
                            if let Some(char_pos) =
                                self.screen_to_visible_position(cursor_position, layout, renderer)
                            {
                                shell.publish(InnerMessage::StartSelection(char_pos));
                            }
                        }
                    }

                    // Handle right-click for context menu
                    if *button == iced::mouse::Button::Right {
                        if let Some(cursor_position) = cursor.position() {
                            shell.publish(InnerMessage::ShowContextMenu(cursor_position));
                        }
                    }

                    shell.capture_event();
                } else {
                    state.unfocus();
                }
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                if self.term.grid.currently_selecting() {
                    if let Some(char_pos) =
                        self.screen_to_visible_position(*position, layout, renderer)
                    {
                        shell.publish(InnerMessage::MoveSelection(char_pos));
                    }
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(button)) => {
                if *button == iced::mouse::Button::Left {
                    if self.term.grid.currently_selecting() {
                        shell.publish(InnerMessage::EndSelection);
                        shell.capture_event();
                    }
                }
            }
            iced::Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                let state = state.state.downcast_mut::<State<Renderer>>();
                let newly_focused = cursor.position_over(layout.bounds()).is_some();

                if newly_focused {
                    state.focus();
                    shell.capture_event();
                } else {
                    state.unfocus();
                }
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                modified_key,
                modifiers,
                ..
            }) => {
                let state = state.state.downcast_mut::<State<Renderer>>();

                if state.is_focused() {
                    if let Some(filter) = &self.term.key_filter {
                        if filter(&modified_key, &modifiers) {
                            return;
                        }
                    }

                    state.last_cursor_blink = Instant::now();
                    state.cursor_blink_currently_shown = true;

                    let message = InnerMessage::KeyPress {
                        modified_key: modified_key.clone(),
                        modifiers: modifiers.clone(),
                    };
                    shell.publish(message);

                    shell.capture_event();
                }
            }
            iced::Event::Window(iced::window::Event::Focused) => {
                let state = state.state.downcast_mut::<State<Renderer>>();
                state.focus();
                shell.request_redraw();
            }
            _ => (),
        }
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let state = tree.state.downcast_mut::<State<Renderer>>();

        state.prerenderer.update(&self.term.grid, renderer);

        iced::advanced::layout::Node::new(limits.max())
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
        let padding_offset =
            iced::Vector::new(self.term.style.padding.left, self.term.style.padding.top);
        let translation = layout.position() - iced::Point::ORIGIN + padding_offset;

        // terminal Background
        renderer.fill_quad(
            iced::advanced::renderer::Quad {
                bounds: layout.bounds(),
                ..Default::default()
            },
            self.term.style.background_color,
        );

        let size = self
            .term
            .style
            .text_size
            .unwrap_or_else(|| renderer.default_size());

        let y_multiplier = self.term.style.line_height.to_absolute(size).0;

        // drawing text background
        for (row_index, render_data) in state.prerenderer.visible_rows().enumerate() {
            let Some((paragraph, spans)) = render_data else {
                continue;
            };
            let y_offset = y_multiplier * row_index as f32;

            for (index, span) in spans.iter().enumerate() {
                if let Some(highlight) = span.highlight {
                    let regions = paragraph.span_bounds(index);

                    for bounds in &regions {
                        let position =
                            bounds.position() - Vector::new(span.padding.left, span.padding.top);

                        let size = bounds.size()
                            + Size::new(span.padding.horizontal(), span.padding.vertical());

                        let position = position + translation + iced::Vector::new(0.0, y_offset);
                        let bounds = Rectangle::new(position, size);

                        renderer.fill_quad(
                            iced::advanced::renderer::Quad {
                                bounds,
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
                self.term.style.foreground_color,
                bounds,
            );
        }

        self.draw_cursor(renderer, &state, translation);
    }
}

impl<'a> TerminalWidget<'a> {
    pub fn new(term: &'a Terminal) -> Self {
        Self { term }
    }

    fn screen_to_visible_position<Renderer>(
        &self,
        screen_pos: iced::Point,
        layout: iced::advanced::Layout<'_>,
        renderer: &Renderer,
    ) -> Option<VisiblePosition>
    where
        Renderer: iced::advanced::text::Renderer,
    {
        let padding_offset =
            iced::Vector::new(self.term.style.padding.left, self.term.style.padding.top);
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
            .style
            .text_size
            .unwrap_or_else(|| renderer.default_size());
        let line_height = self.term.style.line_height.to_absolute(text_size).0;
        let text_size = text_size.0;
        let char_width = text_size * CHAR_WIDTH;

        // Convert to character coordinates
        let char_x = (relative_pos.x / char_width) as usize;
        let char_y = (relative_pos.y / line_height) as usize;

        // Account for scroll offset - the displayed text is offset by scroll_pos
        let absolute_y = char_y;

        Some(VisiblePosition {
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
        let Some(cursor) = self.term.grid.get_cursor() else {
            return;
        };
        if !state.cursor_blink_currently_shown {
            return;
        }

        // Calculate the scroll-adjusted cursor position using your custom scroll system
        // The cursor position is absolute, but we need to adjust it by the scroll offset
        let cursor_absolute_y = cursor.y as i64;
        let scroll_offset = 0 as i64;
        let visible_cursor_y = cursor_absolute_y + scroll_offset;

        // Calculate character dimensions
        let text_size = self
            .term
            .style
            .text_size
            .unwrap_or_else(|| renderer.default_size());

        let line_height = self.term.style.line_height.to_absolute(text_size).0;
        let text_size = text_size.0;
        let char_width = text_size * CHAR_WIDTH;

        let base_cursor_position = iced::Point::new(
            cursor.x as f32 * char_width,
            visible_cursor_y as f32 * line_height,
        );

        let padding = 1.0;

        let cursor_bounds = match self.term.style.cursor_shape {
            CursorShape::Underline => iced::Rectangle::new(
                base_cursor_position
                    + translation
                    + iced::Vector::new(0.0, renderer.default_size().0 * 1.2),
                iced::Size::new(renderer.default_size().0 * CHAR_WIDTH, 1.0),
            ),
            CursorShape::Block => iced::Rectangle::new(
                base_cursor_position + translation + iced::Vector::new(padding, padding),
                iced::Size::new(
                    renderer.default_size().0 * CHAR_WIDTH - padding,
                    renderer.default_size().0 * 1.3 - padding,
                ),
            ),
            CursorShape::Bar => iced::Rectangle::new(
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
