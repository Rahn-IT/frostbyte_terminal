use std::{cell::LazyCell, sync::Arc};

use termwiz::color::ColorAttribute;
use wezterm_term::color::ColorPalette;

use crate::iced::{self, Padding, Pixels};

#[derive(Clone)]
pub struct Style {
    pub line_height: iced::widget::text::LineHeight,
    pub text_size: Option<Pixels>,
    pub padding: iced::Padding,
    pub background_color: iced::Color,
    pub foreground_color: iced::Color,
    pub font: iced::Font,
    pub palette: Arc<Palette256>,
}

pub struct Palette256(pub [iced::Color; 256]);

impl Palette256 {
    fn from_wezterm(palette: wezterm_term::color::Palette256) -> Self {
        let mut iced_palette = [iced::Color::BLACK; 256];

        for (wez_color, iced_color) in palette.0.into_iter().zip(iced_palette.iter_mut()) {
            let (r, g, b, a) = wez_color.to_tuple_rgba();
            *iced_color = iced::Color::from_rgba(r, g, b, a);
        }

        Self(iced_palette)
    }
}

const DEFAULT_STYLE: LazyCell<Style> = LazyCell::new(|| {
    let palette = ColorPalette::default();

    let (r, g, b, a) = palette.background.to_tuple_rgba();
    let background_color = iced::Color::from_rgba(r, g, b, a);

    let (r, g, b, a) = palette.foreground.to_tuple_rgba();
    let foreground_color = iced::Color::from_rgba(r, g, b, a);

    Style {
        line_height: iced::widget::text::LineHeight::default(),
        text_size: None,
        padding: Padding::new(10.0),
        background_color,
        foreground_color,
        font: iced::Font::MONOSPACE,
        palette: Arc::new(Palette256::from_wezterm(palette.colors)),
    }
});

impl Default for Style {
    fn default() -> Self {
        DEFAULT_STYLE.clone()
    }
}

impl Style {
    pub fn line_height(mut self, line_height: impl Into<iced::widget::text::LineHeight>) -> Self {
        self.line_height = line_height.into();
        self
    }

    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn foreground_color(mut self, color: impl Into<iced::Color>) -> Self {
        self.foreground_color = color.into();
        self
    }

    pub fn background_color(mut self, color: impl Into<iced::Color>) -> Self {
        self.background_color = color.into();
        self
    }

    pub fn font(mut self, font: iced::Font) -> Self {
        self.font = font;
        self
    }

    pub fn palette(mut self, palette: Arc<Palette256>) -> Self {
        self.palette = palette;
        self
    }

    pub(crate) fn get_foreground_color(&self, color: ColorAttribute) -> iced::Color {
        self.get_color(color)
            .unwrap_or_else(|| self.foreground_color)
    }

    pub(crate) fn get_background_color(&self, color: ColorAttribute) -> iced::Color {
        self.get_color(color)
            .unwrap_or_else(|| self.background_color)
    }

    fn get_color(&self, color: ColorAttribute) -> Option<iced::Color> {
        match color {
            ColorAttribute::TrueColorWithPaletteFallback(srgba_tuple, _)
            | ColorAttribute::TrueColorWithDefaultFallback(srgba_tuple) => {
                let (r, g, b, a) = srgba_tuple.to_tuple_rgba();
                Some(iced::Color::from_rgba(r, g, b, a))
            }
            ColorAttribute::PaletteIndex(index) => Some(self.palette.0[index as usize]),
            ColorAttribute::Default => None,
        }
    }
}
