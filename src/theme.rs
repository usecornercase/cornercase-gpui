use gpui::{Hsla, Rgba, rgb};
use serde_json::json;

use crate::model::{Colour, Colours, Style};

pub(crate) const FONT_FAMILY: &str = "Menlo";

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

pub(crate) mod colours {
    use super::{Rgba, rgb};
    pub(crate) fn sidebar() -> Rgba {
        rgb(0x16_16_1e)
    }
    pub(crate) fn panel() -> Rgba {
        rgb(0x1f_23_35)
    }
    pub(crate) fn terminal() -> Rgba {
        rgb(0x1a_1b_26)
    }
    pub(crate) fn border() -> Rgba {
        rgb(0x29_2e_42)
    }
    pub(crate) fn hover() -> Rgba {
        rgb(0x29_2e_42)
    }
    pub(crate) fn selected() -> Rgba {
        rgb(0x2f_35_49)
    }
    pub(crate) fn text() -> Rgba {
        rgb(0xc0_ca_f5)
    }
    pub(crate) fn muted() -> Rgba {
        rgb(0x73_7a_a2)
    }
    pub(crate) fn faint() -> Rgba {
        rgb(0x56_5f_89)
    }
    pub(crate) fn accent() -> Rgba {
        rgb(0x7a_a2_f7)
    }
    pub(crate) fn working() -> Rgba {
        rgb(0xe0_af_68)
    }
    pub(crate) fn waiting() -> Rgba {
        rgb(0xff_9e_64_u32)
    }
    pub(crate) fn done() -> Rgba {
        rgb(0x9e_ce_6a)
    }
    pub(crate) fn failed() -> Rgba {
        rgb(0xf7_76_8e)
    }
}

pub(crate) fn status_badge(status: Option<&str>) -> Option<(&'static str, Rgba)> {
    match status? {
        "working" => Some(("◐", colours::working())),
        "waiting" => Some(("!", colours::waiting())),
        "done" => Some(("✓", colours::done())),
        "failed" => Some(("✗", colours::failed())),
        "pending" => Some(("●", colours::accent())),
        "idle" => Some(("○", colours::faint())),
        _ => None,
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

    pub(crate) fn underline_colour(&self, style: &Style) -> Option<Hsla> {
        style.underline_colour.map(|c| hsla(self.colour(c)))
    }

    pub(crate) fn cursor(&self) -> Hsla {
        hsla(self.colours.cursor.unwrap_or(self.default_fg()))
    }
}
