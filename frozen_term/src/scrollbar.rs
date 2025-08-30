use iced::{
    Background, Color, Length, Pixels, Point, Rectangle, Size,
    advanced::{Widget, renderer::Quad},
    widget::scrollable,
};

pub struct Scrollbar<T, Message> {
    count: T,
    scroll: T,
    window: T,
    on_scroll: Option<Box<dyn Fn(Pixels) -> Message>>,
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
}

const WIDTH: f32 = 20.0;

struct State {
    scroller_grabbed_at: Option<Point>,
    status: Option<scrollable::Status>,
    scroller_position_mult: f32,
    scroller_height_mult: f32,
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
            scroller_position_mult: 0.0,
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

        state.scroller_position_mult = scroll.0 / count.0;
        state.scroller_height_mult = (window.0 / count.0).min(1.0);

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
                    state.scroller_position_mult,
                    state.scroller_height_mult,
                );
                if let Some(position) = cursor.position_over(scroller_rect) {
                    state.scroller_grabbed_at = Some(position);
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
                        state.scroller_position_mult,
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
                if let Some(grabbed_at) = state.scroller_grabbed_at {
                    todo!()
                } else {
                    let scroller_rect = scroller_rect(
                        layout,
                        state.scroller_position_mult,
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
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();

        let scroller = scroller_rect(
            layout,
            state.scroller_position_mult,
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
    scroller_position_mult: f32,
    scroller_height_mult: f32,
) -> Rectangle {
    let mut bounds = layout.bounds();
    bounds.y = bounds.y + bounds.height * scroller_position_mult;
    bounds.height = bounds.height * scroller_height_mult;
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
