use iced::{
    Background, Color, Length, Pixels, Point, Rectangle, Size,
    advanced::{Widget, renderer::Quad},
    widget::scrollable,
};

pub struct Scrollbar<T, Message> {
    count: T,
    scroll: T,
    window: T,
    on_scroll: Option<Box<dyn Fn(f32) -> Message>>,
}

impl<T, Message> Scrollbar<T, Message>
where
    T: Into<Pixels>,
{
    pub fn new(count: T, scroll: T, window: T) -> Self {
        Scrollbar {
            count,
            scroll,
            window,
            on_scroll: None,
        }
    }

    pub fn on_scroll(mut self, on_scroll: impl Fn(f32) -> Message + 'static) -> Self {
        self.on_scroll = Some(Box::new(on_scroll));
        self
    }
}

const WIDTH: f32 = 20.0;

struct State {
    scroller_grabbed_at: Option<Grab>,
    status: Option<scrollable::Status>,
    relative_scroll: f32,
    scroller_space_mult: f32,
    scroller_height_mult: f32,
}

struct Grab {
    at: Point,
    relative_scroll: f32,
}

impl<T, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Scrollbar<T, Message>
where
    Renderer: iced::advanced::Renderer,
    Theme: iced::widget::scrollable::Catalog,
    T: Into<Pixels> + Clone,
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State {
            scroller_grabbed_at: None,
            status: None,
            relative_scroll: 0.0,
            scroller_space_mult: 0.0,
            scroller_height_mult: 1.0,
        })
    }

    fn size(&self) -> iced::Size<iced::Length> {
        Size::new(Length::Shrink, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        _renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let state = tree.state.downcast_mut::<State>();
        let count: Pixels = self.count.clone().into();
        let scroll: Pixels = self.scroll.clone().into();
        let window: Pixels = self.window.clone().into();

        state.scroller_height_mult = (window.0 / count.0).min(1.0);
        state.relative_scroll = scroll.0 / count.0;
        let space_left = count.0 - window.0;
        state.scroller_space_mult = if space_left > 0.0 {
            scroll.0 / space_left
        } else {
            0.0
        };

        iced::advanced::layout::Node::new(Size::new(WIDTH.into(), limits.max().height))
    }

    fn update(
        &mut self,
        state: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) {
        match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                let state = state.state.downcast_mut::<State>();
                let scroller_rect = scroller_rect(
                    layout,
                    state.scroller_space_mult,
                    state.scroller_height_mult,
                );
                if let Some(position) = cursor.position_over(scroller_rect) {
                    state.scroller_grabbed_at = Some(Grab {
                        at: position,
                        relative_scroll: state.relative_scroll,
                    });
                    state.status = Some(scrollable::Status::Dragged {
                        is_horizontal_scrollbar_dragged: false,
                        is_vertical_scrollbar_dragged: true,
                        is_horizontal_scrollbar_disabled: true,
                        is_vertical_scrollbar_disabled: false,
                    });
                    shell.request_redraw();
                }
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                let state = state.state.downcast_mut::<State>();
                if state.scroller_grabbed_at.is_some() {
                    state.scroller_grabbed_at = None;
                    let scroller_rect = scroller_rect(
                        layout,
                        state.scroller_space_mult,
                        state.scroller_height_mult,
                    );
                    let new_status = if cursor.position_over(scroller_rect).is_some() {
                        Some(scrollable::Status::Hovered {
                            is_horizontal_scrollbar_hovered: false,
                            is_vertical_scrollbar_hovered: true,
                            is_horizontal_scrollbar_disabled: true,
                            is_vertical_scrollbar_disabled: false,
                        })
                    } else {
                        Some(scrollable::Status::Active {
                            is_horizontal_scrollbar_disabled: true,
                            is_vertical_scrollbar_disabled: false,
                        })
                    };

                    state.status = new_status;
                    shell.request_redraw();
                }
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. }) => {
                let state = state.state.downcast_mut::<State>();
                if let Some(((grabbed, cursor_pos), on_scroll)) = state
                    .scroller_grabbed_at
                    .as_ref()
                    .zip(cursor.position())
                    .zip(self.on_scroll.as_ref())
                {
                    let height_diff = cursor_pos.y - grabbed.at.y;
                    let relative_scroll_delta = height_diff / layout.bounds().height;
                    let relative_scroll = grabbed.relative_scroll + relative_scroll_delta;

                    shell.publish(on_scroll(relative_scroll.max(0.0).min(1.0)));
                } else {
                    let scroller_rect = scroller_rect(
                        layout,
                        state.scroller_space_mult,
                        state.scroller_height_mult,
                    );
                    let new_status = if cursor.position_over(scroller_rect).is_some() {
                        Some(scrollable::Status::Hovered {
                            is_horizontal_scrollbar_hovered: false,
                            is_vertical_scrollbar_hovered: true,
                            is_horizontal_scrollbar_disabled: true,
                            is_vertical_scrollbar_disabled: false,
                        })
                    } else {
                        Some(scrollable::Status::Active {
                            is_horizontal_scrollbar_disabled: true,
                            is_vertical_scrollbar_disabled: false,
                        })
                    };

                    if new_status != state.status {
                        state.status = new_status;
                        shell.request_redraw();
                    }
                };
            }
            _ => (),
        }
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();

        let scroller = scroller_rect(
            layout,
            state.scroller_space_mult,
            state.scroller_height_mult,
        );

        let style = theme.style(
            &Theme::default(),
            state.status.unwrap_or_else(|| scrollable::Status::Active {
                is_horizontal_scrollbar_disabled: true,
                is_vertical_scrollbar_disabled: false,
            }),
        );

        // Draw the scrollbar background
        renderer.fill_quad(
            Quad {
                bounds: layout.bounds(),
                border: style.vertical_rail.border,
                ..Quad::default()
            },
            style
                .vertical_rail
                .background
                .unwrap_or(Background::Color(Color::TRANSPARENT)),
        );

        // Draw the scroller
        renderer.fill_quad(
            Quad {
                bounds: scroller,
                border: style.vertical_rail.scroller.border,
                ..Quad::default()
            },
            style.vertical_rail.scroller.color,
        );
    }
}

fn scroller_rect(
    layout: iced::advanced::Layout<'_>,
    scroller_space_mult: f32,
    scroller_height_mult: f32,
) -> Rectangle {
    let mut bounds = layout.bounds();
    let min_height = bounds.width;

    let calculated_height = (bounds.height * scroller_height_mult).max(min_height);
    let space_left = bounds.height - calculated_height;
    let calculated_position = space_left * scroller_space_mult;

    bounds.y += calculated_position;
    bounds.height = calculated_height;
    bounds
}

impl<'a, T, Message, Theme, Renderer> From<Scrollbar<T, Message>>
    for iced::Element<'a, Message, Theme, Renderer>
where
    T: Into<Pixels> + 'static + Clone,
    Renderer: iced::advanced::Renderer,
    Theme: scrollable::Catalog,
    Message: 'static,
{
    fn from(scrollbar: Scrollbar<T, Message>) -> Self {
        iced::Element::new(scrollbar)
    }
}
