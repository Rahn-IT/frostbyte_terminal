use iced::advanced::text::{self};

pub trait TerminalGrid {
    fn advance_bytes(&mut self, bytes: &[u8]);
    fn resize(&mut self, size: Size);

    fn press_key(
        &mut self,
        key: iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    ) -> Option<Vec<u8>>;
    fn paste(&mut self, text: &str) -> Option<Vec<u8>>;

    fn scroll(&mut self, lines: isize);
    fn get_scroll(&self) -> usize;
    fn available_lines(&self) -> usize;

    fn start_selection(&mut self, start: VisiblePosition);
    fn move_selection(&mut self, end: VisiblePosition);
    fn end_selection(&mut self);
    fn currently_selecting(&self) -> bool;
    fn selected_text(&self) -> Option<String>;

    fn get_title(&self) -> &str;
    fn get_size(&self) -> Size;
    fn get_cursor(&self) -> Option<VisiblePosition>;
}

pub trait PreRenderer<R>
where
    R: text::Renderer,
    R::Font: 'static,
{
    type Grid: TerminalGrid;

    fn update(&mut self, grid: &Self::Grid, renderer: &R);
    fn visible_rows<'a>(
        &'a self,
    ) -> impl Iterator<Item = Option<(&'a R::Paragraph, &'a [text::Span<'a, (), R::Font>])>>;
}

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub cols: usize,
    pub rows: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisiblePosition {
    pub x: usize,
    pub y: usize,
}
