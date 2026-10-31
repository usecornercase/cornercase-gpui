use std::sync::LazyLock;

use gpui::{Hsla, Rgba, rgb, rgba};
use serde_json::json;

use crate::model::{Colour, Colours, Style};

pub(crate) const FONT_FAMILY: &str = "Menlo";
const OPACITY: f32 = 0.65;
const LEAST_OPACITY: f32 = 0.3;
pub(crate) const INACTIVE: f32 = 0.55;

static TERMINAL_OPACITY: LazyLock<f32> =
    LazyLock::new(|| opacity_from(std::env::var("CORNERCASE_GPUI_OPACITY").ok().as_deref()));

pub(crate) const FOREGROUND: [u8; 3] = [192, 202, 245];
pub(crate) const BACKGROUND: [u8; 3] = [26, 27, 38];
const BLACK: [u8; 3] = [21, 22, 30];
const RED: [u8; 3] = [247, 118, 142];
const GREEN: [u8; 3] = [158, 206, 106];
const YELLOW: [u8; 3] = [224, 175, 104];
const BLUE: [u8; 3] = [122, 162, 247];
const MAGENTA: [u8; 3] = [187, 154, 247];
const CYAN: [u8; 3] = [125, 207, 255];
const WHITE: [u8; 3] = [169, 177, 214];
const BRIGHT_BLACK: [u8; 3] = [86, 95, 137];
pub(crate) const ANSI: [[u8; 3]; 16] = [
    BLACK,
    RED,
    GREEN,
    YELLOW,
    BLUE,
    MAGENTA,
    CYAN,
    WHITE,
    BRIGHT_BLACK,
    RED,
    GREEN,
    YELLOW,
    BLUE,
    MAGENTA,
    CYAN,
    FOREGROUND,
];

pub(crate) mod radius {
    pub(crate) const CHIP: f32 = 5.;
    pub(crate) const CONTROL: f32 = 8.;
    pub(crate) const CARD: f32 = 12.;
    pub(crate) const SHEET: f32 = 16.;
}

pub(crate) mod colours {
    use super::{Rgba, rgb, rgba};
    pub(crate) fn brand() -> Rgba {
        rgb(0x00_5f_ff)
    }
    pub(crate) fn brand_hover() -> Rgba {
        rgb(0x00_50_d8)
    }
    pub(crate) fn brand_light() -> Rgba {
        rgb(0x2b_7b_ff)
    }
    pub(crate) fn on_brand() -> Rgba {
        rgb(0xf7_f8_fa)
    }
    pub(crate) fn accent() -> Rgba {
        rgb(0x74_a3_ff)
    }
    pub(crate) fn accent_soft() -> Rgba {
        rgba(0x00_5f_ff_2e)
    }
    pub(crate) fn selection() -> Rgba {
        rgba(0x00_5f_ff_47)
    }
    pub(crate) fn glass() -> Rgba {
        rgba(0x0b_0c_0e_8c)
    }
    pub(crate) fn veil() -> Rgba {
        rgba(0x0a_0b_10_9e)
    }
    pub(crate) fn sidebar() -> Rgba {
        rgba(0x00_00_00_40)
    }
    pub(crate) fn panel() -> Rgba {
        rgba(0xff_ff_ff_0a)
    }
    pub(crate) fn raised() -> Rgba {
        rgb(0x1d_20_25)
    }
    pub(crate) fn raised_hover() -> Rgba {
        rgb(0x2a_2e_35)
    }
    pub(crate) fn field() -> Rgba {
        rgba(0x05_06_08_8c)
    }
    pub(crate) fn terminal() -> Rgba {
        Rgba { a: super::terminal_opacity(), ..rgb(0x1a_1b_26) }
    }
    pub(crate) fn pane() -> Rgba {
        super::over_ground(terminal(), glass())
    }
    pub(crate) fn surface() -> Rgba {
        rgba(0x2a_2e_35_4d)
    }
    pub(crate) fn ink() -> Rgba {
        rgb(0x0b_0c_0e)
    }
    pub(crate) fn border() -> Rgba {
        rgba(0xff_ff_ff_17)
    }
    pub(crate) fn edge() -> Rgba {
        rgba(0xff_ff_ff_2b)
    }
    pub(crate) fn hover() -> Rgba {
        rgba(0xff_ff_ff_0f)
    }
    pub(crate) fn selected() -> Rgba {
        rgba(0xff_ff_ff_1a)
    }
    pub(crate) fn text() -> Rgba {
        rgb(0xec_ee_f1)
    }
    pub(crate) fn soft() -> Rgba {
        rgb(0xb6_bc_c4)
    }
    pub(crate) fn muted() -> Rgba {
        rgb(0x8b_92_9b)
    }
    pub(crate) fn faint() -> Rgba {
        rgb(0x6b_72_7c)
    }
    pub(crate) fn working() -> Rgba {
        rgb(0xf5_d7_6e)
    }
    pub(crate) fn waiting() -> Rgba {
        rgb(0xff_87_00_u32)
    }
    pub(crate) fn done() -> Rgba {
        rgb(0x58_e6_a0)
    }
    pub(crate) fn failed() -> Rgba {
        rgb(0xff_6b_8b)
    }
}

pub(crate) fn palette(index: u8) -> [u8; 3] {
    match index {
        0..=15 => ANSI[index as usize],
        16..=231 => {
            let i = index - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            [level(i / 36), level((i / 6) % 6), level(i % 6)]
        }
        _ => {
            let v = 8 + (index - 232) * 10;
            [v, v, v]
        }
    }
}

pub(crate) fn opacity_from(value: Option<&str>) -> f32 {
    value
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .map_or(OPACITY, |v| v.clamp(LEAST_OPACITY, 1.))
}

pub(crate) fn terminal_opacity() -> f32 {
    *TERMINAL_OPACITY
}

pub(crate) fn over_ground(wanted: Rgba, ground: Rgba) -> Rgba {
    let a = (1. - (1. - wanted.a) / (1. - ground.a)).clamp(0., 1.);
    if a <= 0. {
        return Rgba { a: 0., ..wanted };
    }
    let mix = |c: f32, g: f32| ((c * wanted.a - g * ground.a * (1. - a)) / a).clamp(0., 1.);
    Rgba { r: mix(wanted.r, ground.r), g: mix(wanted.g, ground.g), b: mix(wanted.b, ground.b), a }
}

pub(crate) fn faded(colour: Hsla, by: f32) -> Hsla {
    Hsla { a: colour.a * by, ..colour }
}

pub(crate) fn theme_json() -> String {
    let palette: Vec<[u8; 3]> = (0..=255u8).map(palette).collect();
    json!({ "foreground": FOREGROUND, "background": BACKGROUND, "palette": palette }).to_string()
}

pub(crate) fn hsla([r, g, b]: [u8; 3]) -> Hsla {
    Rgba { r: f32::from(r) / 255., g: f32::from(g) / 255., b: f32::from(b) / 255., a: 1. }.into()
}

pub(crate) struct Painter<'a> {
    pub colours: &'a Colours,
}

impl Painter<'_> {
    pub(crate) fn default_fg(&self) -> [u8; 3] {
        self.colours.fg.unwrap_or(FOREGROUND)
    }

    pub(crate) fn default_bg(&self) -> [u8; 3] {
        self.colours.bg.unwrap_or(BACKGROUND)
    }

    fn colour(&self, colour: Colour) -> [u8; 3] {
        match colour {
            Colour::Rgb(rgb) => rgb,
            Colour::Index(i) => self.colours.palette.get(&i.to_string()).copied().unwrap_or_else(|| palette(i)),
        }
    }

    pub(crate) fn resolve(&self, style: &Style) -> (Hsla, Hsla, bool) {
        let mut fg = style.fg.map_or(self.default_fg(), |c| self.colour(c));
        let mut bg = style.bg.map_or(self.default_bg(), |c| self.colour(c));
        let mut custom_bg = style.bg.is_some();
        if style.inverse {
            std::mem::swap(&mut fg, &mut bg);
            custom_bg = true;
        }
        let mut fg = hsla(fg);
        if style.dim {
            fg.a = 0.6;
        }
        if style.invisible {
            fg.a = 0.;
        }
        (fg, hsla(bg), custom_bg)
    }

    pub(crate) fn pane_fill(&self) -> Option<Hsla> {
        let bg = self.default_bg();
        (bg != BACKGROUND).then(|| hsla(bg))
    }

    pub(crate) fn cell_fill(&self, bg: Hsla, custom: bool) -> Option<Hsla> {
        (custom && bg != hsla(self.default_bg())).then_some(bg)
    }

    pub(crate) fn underline_colour(&self, style: &Style) -> Option<Hsla> {
        style.underline_colour.map(|c| hsla(self.colour(c)))
    }

    pub(crate) fn cursor(&self) -> Hsla {
        hsla(self.colours.cursor.unwrap_or(self.default_fg()))
    }
}

#[cfg(test)]
mod tests;
