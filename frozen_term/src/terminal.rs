use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use iced::{
    Border, Color, Element, Font, Length, Point, Rectangle, Size, Task, Vector,
    advanced::{
        Shell, Text,
        graphics::core::widget,
        layout::Node,
        renderer::Quad,
        text::{Paragraph, Renderer},
        widget::{
            operate,
            operation::{Focusable, focusable::focus},
        },
    },
    alignment::{Horizontal, Vertical},
    keyboard,
    task::Handle,
    widget::text::{LineHeight, Shaping, Wrapping},
    window::RedrawRequest,
};
use termwiz::surface::{CursorShape, CursorVisibility};
use tokio::sync::mpsc;
use wezterm_term::{
    CellAttributes, CursorPosition, TerminalConfiguration,
    color::{ColorAttribute, ColorPalette},
};

pub use wezterm_term::TerminalSize;

#[derive(Debug, Clone)]
pub struct MessageWrapper(Message);

#[derive(Debug, Clone)]
pub enum Message {
    Resize(TerminalSize),
    KeyPress {
        modified_key: keyboard::key::Key,
        modifiers: keyboard::Modifiers,
    },
    Input(Vec<u8>),
    Paste(Option<String>),
}

pub enum Action {
    None,
    Run(Task<MessageWrapper>),
    Resize(TerminalSize),
    Input(Vec<u8>),
}

pub struct Terminal {
    term: wezterm_term::Terminal,
    id: Option<Id>,
    key_filter: Option<Box<dyn Fn(&iced::keyboard::Key, &iced::keyboard::Modifiers) -> bool>>,
    // here to abort the task on drop
    _handle: Handle,
    font: Font,
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
    pub fn new(rows: u16, cols: u16) -> (Self, Task<MessageWrapper>) {
        let size = TerminalSize {
            rows: rows as usize,
            cols: cols as usize,
            ..Default::default()
        };

        let config = Config {};

        let (send, recv) = mpsc::channel(10);
        let recv = tokio_stream::wrappers::ReceiverStream::new(recv);
        let writer = BridgedWriter { send };

        let term = wezterm_term::Terminal::new(
            size,
            Arc::new(config),
            "frozen_term",
            "0.1",
            Box::new(writer),
        );

        let (task, handle) = Task::run(recv, Message::Input)
            .map(MessageWrapper)
            .abortable();

        let handle = handle.abort_on_drop();

        (
            Self {
                term,
                id: None,
                _handle: handle,
                key_filter: None,
                font: Font::MONOSPACE,
            },
            task,
        )
    }

    pub fn id(mut self, id: impl Into<Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn random_id(self) -> Self {
        self.id(Id(widget::Id::unique()))
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

    pub fn font(mut self, font: impl Into<Font>) -> Self {
        self.font = font.into();
        self
    }

    pub fn focus<T>(&self) -> Task<T>
    where
        T: Send + 'static,
    {
        if let Some(id) = &self.id {
            Self::focus_with_id(id.clone())
        } else {
            Task::none()
        }
    }

    pub fn focus_with_id<T>(id: Id) -> Task<T>
    where
        T: Send + 'static,
    {
        operate(focus(id.0))
    }

    pub fn get_title(&self) -> &str {
        self.term.get_title()
    }

    pub fn advance_bytes<B>(&mut self, bytes: B)
    where
        B: AsRef<[u8]>,
    {
        self.term.advance_bytes(bytes)
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
                    return Action::Run(
                        iced::clipboard::read()
                            .map(Message::Paste)
                            .map(MessageWrapper),
                    );
                }

                if let Some((key, modifiers)) = transform_key(modified_key, modifiers) {
                    self.term.key_down(key, modifiers).unwrap();
                }

                Action::None
            }
            Message::Input(input) => Action::Input(input),
            Message::Paste(paste) => {
                if let Some(paste) = paste {
                    self.term.send_paste(&paste).unwrap();
                }
                Action::None
            }
        }
    }

    pub fn view<'a, Theme, Renderer>(&'a self) -> Element<'a, MessageWrapper, Theme, Renderer>
    where
        Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'static,
        Theme: iced::widget::text::Catalog + 'static,
        Theme: iced::widget::container::Catalog,
        <Theme as iced::widget::text::Catalog>::Class<'static>:
            From<iced::widget::text::StyleFn<'static, Theme>>,
        <Theme as iced::widget::container::Catalog>::Class<'static>:
            From<iced::widget::container::StyleFn<'static, Theme>>,
    {
        Element::new(TerminalWidget::new(self, self.font).id_maybe(self.id.clone()))
            .map(MessageWrapper)
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
            keyboard::key::Named::Enter => Some(wezterm_term::KeyCode::Enter),
            keyboard::key::Named::Space => Some(wezterm_term::KeyCode::Char(' ')),
            keyboard::key::Named::Backspace => Some(wezterm_term::KeyCode::Backspace),
            keyboard::key::Named::Delete => Some(wezterm_term::KeyCode::Delete),
            keyboard::key::Named::ArrowLeft => Some(wezterm_term::KeyCode::LeftArrow),
            keyboard::key::Named::ArrowRight => Some(wezterm_term::KeyCode::RightArrow),
            keyboard::key::Named::ArrowUp => Some(wezterm_term::KeyCode::UpArrow),
            keyboard::key::Named::ArrowDown => Some(wezterm_term::KeyCode::DownArrow),
            keyboard::key::Named::Tab => Some(wezterm_term::KeyCode::Tab),
            keyboard::key::Named::Escape => Some(wezterm_term::KeyCode::Escape),
            keyboard::key::Named::F1 => Some(wezterm_term::KeyCode::Function(1)),
            keyboard::key::Named::F2 => Some(wezterm_term::KeyCode::Function(2)),
            keyboard::key::Named::F3 => Some(wezterm_term::KeyCode::Function(3)),
            keyboard::key::Named::F4 => Some(wezterm_term::KeyCode::Function(4)),
            keyboard::key::Named::F5 => Some(wezterm_term::KeyCode::Function(5)),
            keyboard::key::Named::F6 => Some(wezterm_term::KeyCode::Function(6)),
            keyboard::key::Named::F7 => Some(wezterm_term::KeyCode::Function(7)),
            keyboard::key::Named::F8 => Some(wezterm_term::KeyCode::Function(8)),
            keyboard::key::Named::F9 => Some(wezterm_term::KeyCode::Function(9)),
            keyboard::key::Named::F10 => Some(wezterm_term::KeyCode::Function(10)),
            keyboard::key::Named::F11 => Some(wezterm_term::KeyCode::Function(11)),
            keyboard::key::Named::F12 => Some(wezterm_term::KeyCode::Function(12)),
            keyboard::key::Named::F13 => Some(wezterm_term::KeyCode::Function(13)),
            keyboard::key::Named::F14 => Some(wezterm_term::KeyCode::Function(14)),
            keyboard::key::Named::F15 => Some(wezterm_term::KeyCode::Function(15)),
            keyboard::key::Named::F16 => Some(wezterm_term::KeyCode::Function(16)),
            keyboard::key::Named::F17 => Some(wezterm_term::KeyCode::Function(17)),
            keyboard::key::Named::F18 => Some(wezterm_term::KeyCode::Function(18)),
            keyboard::key::Named::F19 => Some(wezterm_term::KeyCode::Function(19)),
            keyboard::key::Named::F20 => Some(wezterm_term::KeyCode::Function(20)),
            keyboard::key::Named::F21 => Some(wezterm_term::KeyCode::Function(21)),
            keyboard::key::Named::F22 => Some(wezterm_term::KeyCode::Function(22)),
            keyboard::key::Named::F23 => Some(wezterm_term::KeyCode::Function(23)),
            keyboard::key::Named::F24 => Some(wezterm_term::KeyCode::Function(24)),
            keyboard::key::Named::F25 => Some(wezterm_term::KeyCode::Function(25)),
            keyboard::key::Named::F26 => Some(wezterm_term::KeyCode::Function(26)),
            keyboard::key::Named::F27 => Some(wezterm_term::KeyCode::Function(27)),
            keyboard::key::Named::F28 => Some(wezterm_term::KeyCode::Function(28)),
            keyboard::key::Named::F29 => Some(wezterm_term::KeyCode::Function(29)),
            keyboard::key::Named::F30 => Some(wezterm_term::KeyCode::Function(30)),
            keyboard::key::Named::F31 => Some(wezterm_term::KeyCode::Function(31)),
            keyboard::key::Named::F32 => Some(wezterm_term::KeyCode::Function(32)),
            keyboard::key::Named::F33 => Some(wezterm_term::KeyCode::Function(33)),
            keyboard::key::Named::F34 => Some(wezterm_term::KeyCode::Function(34)),
            keyboard::key::Named::F35 => Some(wezterm_term::KeyCode::Function(35)),
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
}

struct State<R: Renderer> {
    focused: bool,
    paragraph: R::Paragraph,
    spans: Vec<iced::advanced::text::Span<'static, (), R::Font>>,
    last_render_seqno: usize,
    cursor: CursorPosition,
    last_cursor_blink: Instant,
    now: Instant,
}

const CHAR_WIDTH: f32 = 0.6;
const CURSOR_BLINK_INTERVAL_MILLIS: u128 = 500;

impl<Renderer> Focusable for State<Renderer>
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

impl<Theme, Renderer> iced::advanced::widget::Widget<Message, Theme, Renderer>
    for TerminalWidget<'_, Renderer>
where
    Renderer: iced::advanced::text::Renderer,
    Renderer: 'static,
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State<Renderer>>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State::<Renderer> {
            focused: false,
            paragraph: Renderer::Paragraph::default(),
            spans: Vec::new(),
            last_render_seqno: 0,
            cursor: CursorPosition::default(),
            last_cursor_blink: Instant::now(),
            now: Instant::now(),
        })
    }

    fn size(&self) -> iced::Size<iced::Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn operate(
        &self,
        tree: &mut iced::advanced::widget::Tree,
        _layout: iced::advanced::Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let state = tree.state.downcast_mut::<State<Renderer>>();

        operation.focusable(state, self.id.as_ref().map(|id| &id.0));
    }

    fn layout(
        &self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let state = tree.state.downcast_mut::<State<Renderer>>();
        let term = &self.term.term;
        let current_seqno = term.current_seqno();

        if state.last_render_seqno != current_seqno {
            let screen = term.screen();

            let range = screen.phys_range(&(0..screen.physical_rows as i64));
            let term_lines = screen.lines_in_phys_range(range);

            let mut current_text = String::new();
            let mut current_attrs = CellAttributes::default();
            state.spans.clear();

            let palette = term.palette();

            state.cursor = term.cursor_pos();

            for line in term_lines.iter() {
                for cell in line.visible_cells() {
                    if cell.attrs() != &current_attrs {
                        if !current_text.is_empty() {
                            let foreground = get_color(current_attrs.foreground(), &palette);
                            let background = get_color(current_attrs.background(), &palette);

                            let span = iced::advanced::text::Span::new(current_text.clone())
                                .color_maybe(foreground)
                                .background_maybe(background);

                            state.spans.push(span);
                            current_text.clear();
                        }
                        current_attrs = cell.attrs().clone();
                    }

                    current_text.push_str(cell.str());
                }
                current_text.push('\n');
            }

            if current_text.len() > 1 {
                let foreground = get_color(current_attrs.foreground(), &palette);
                let background = get_color(current_attrs.background(), &palette);

                let span = iced::advanced::text::Span::new(current_text)
                    .color_maybe(foreground)
                    .background_maybe(background);

                state.spans.push(span);
            }

            let text = Text {
                content: state.spans.as_ref(),
                bounds: limits.max(),
                size: renderer.default_size(),
                line_height: LineHeight::default(),
                font: self.font,
                horizontal_alignment: Horizontal::Left,
                vertical_alignment: Vertical::Top,
                shaping: Shaping::Advanced,
                wrapping: Wrapping::None,
            };

            state.paragraph = Paragraph::with_spans(text);
        }

        Node::new(limits.max())
    }

    fn on_event(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: iced::Event,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) -> iced::advanced::graphics::core::event::Status {
        match event {
            iced::Event::Window(iced::window::Event::RedrawRequested(now)) => {
                let term = &self.term.term;
                let screen = term.screen();

                let widget_width = layout.bounds().width;
                let widget_height = layout.bounds().height;
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
                    state.now = now;
                    let millis_until_redraw = CURSOR_BLINK_INTERVAL_MILLIS
                        - (now - state.last_cursor_blink).as_millis()
                            % CURSOR_BLINK_INTERVAL_MILLIS;

                    shell.request_redraw(RedrawRequest::At(
                        now + Duration::from_millis(millis_until_redraw as u64),
                    ));
                }

                iced::advanced::graphics::core::event::Status::Ignored
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
            | iced::Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                let state = tree.state.downcast_mut::<State<Renderer>>();

                state.focused = true;

                iced::advanced::graphics::core::event::Status::Captured
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

                    let message = Message::KeyPress {
                        modified_key,
                        modifiers,
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
        let translation = layout.position() - Point::ORIGIN;

        for (index, span) in state.spans.iter().enumerate() {
            if let Some(highlight) = span.highlight {
                let regions = state.paragraph.span_bounds(index);

                for bounds in &regions {
                    let bounds = Rectangle::new(
                        bounds.position() - Vector::new(span.padding.left, span.padding.top),
                        bounds.size()
                            + Size::new(span.padding.horizontal(), span.padding.vertical()),
                    );

                    renderer.fill_quad(
                        Quad {
                            bounds: bounds + translation,
                            border: highlight.border,
                            ..Default::default()
                        },
                        highlight.background,
                    );
                }
            }
        }

        draw_cursor(renderer, &state, translation);

        renderer.fill_paragraph(&state.paragraph, bounds.position(), Color::WHITE, bounds);
    }
}

fn draw_cursor<Renderer>(
    renderer: &mut Renderer,
    state: &State<Renderer>,
    translation: iced::Vector,
) where
    Renderer: iced::advanced::text::Renderer,
{
    let is_cursor_visible = state.cursor.visibility == CursorVisibility::Visible
        && ((state.now - state.last_cursor_blink).as_millis() / CURSOR_BLINK_INTERVAL_MILLIS) % 2
            == 0;

    if !is_cursor_visible {
        return;
    }

    let base_cursor_position = Point::new(
        state.cursor.x as f32 * renderer.default_size().0 * CHAR_WIDTH,
        state.cursor.y as f32 * renderer.default_size().0 * 1.3,
    );

    let padding = 1.0;

    let cursor_bounds = match state.cursor.shape {
        CursorShape::BlinkingUnderline | CursorShape::SteadyUnderline | CursorShape::Default => {
            Rectangle::new(
                base_cursor_position
                    + translation
                    + Vector::new(0.0, renderer.default_size().0 * 1.2),
                Size::new(renderer.default_size().0 * CHAR_WIDTH, 1.0),
            )
        }
        CursorShape::BlinkingBlock | CursorShape::SteadyBlock => Rectangle::new(
            base_cursor_position + translation + Vector::new(padding, padding),
            Size::new(
                renderer.default_size().0 * CHAR_WIDTH - padding,
                renderer.default_size().0 * 1.3 - padding,
            ),
        ),
        CursorShape::BlinkingBar | CursorShape::SteadyBar => Rectangle::new(
            base_cursor_position + translation + Vector::new(padding, padding),
            Size::new(1.0, renderer.default_size().0 * 1.3 - padding),
        ),
    };

    renderer.fill_quad(
        Quad {
            bounds: cursor_bounds,
            border: Border::default(),
            ..Default::default()
        },
        Color::WHITE,
    );
}
