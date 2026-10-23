//! Colours: the terminal palette (also given to cornercase with `cornercase theme`, so programs
//! pick a look that matches what we draw) and the chrome around it.

use gpui::{Hsla, Rgba, rgb};
use serde_json::json;

use crate::model::{Colour, Colours, Style};

pub(crate) const FONT_FAMILY: &str = "Menlo";

/// Tokyo Night, the dark theme cornercase's own fixtures use.
pub(crate) const FOREGROUND: [u8; 3] = [192, 202, 245];
pub(crate) const BACKGROUND: [u8; 3] = [26, 27, 38];
pub(crate) const ANSI: [[u8; 3]; 16] = [
    [21, 22, 30],
    [247, 118, 142],
    [158, 206, 106],
    [224, 175, 104],
    [122, 162, 247],
    [187, 154, 247],
    [125, 207, 255],
    [169, 177, 214],
    [86, 95, 137],
    [247, 118, 142],
    [158, 206, 106],
    [224, 175, 104],
    [122, 162, 247],
    [187, 154, 247],
    [125, 207, 255],
    [192, 202, 245],
];

/// The chrome's colours.
pub(crate) mod colours {
    use super::{Rgba, rgb};
    pub(crate) fn sidebar() -> Rgba {
        rgb(0x16161e)
    }
    pub(crate) fn panel() -> Rgba {
        rgb(0x1f2335)
    }
    pub(crate) fn terminal() -> Rgba {
        rgb(0x1a1b26)
    }
    pub(crate) fn border() -> Rgba {
        rgb(0x292e42)
    }
    pub(crate) fn hover() -> Rgba {
        rgb(0x292e42)
    }
    pub(crate) fn selected() -> Rgba {
        rgb(0x2f3549)
    }
    pub(crate) fn text() -> Rgba {
        rgb(0xc0caf5)
    }
    pub(crate) fn muted() -> Rgba {
        rgb(0x737aa2)
    }
    pub(crate) fn faint() -> Rgba {
        rgb(0x565f89)
    }
    pub(crate) fn accent() -> Rgba {
        rgb(0x7aa2f7)
    }
    pub(crate) fn working() -> Rgba {
        rgb(0xe0af68)
    }
    pub(crate) fn waiting() -> Rgba {
        rgb(0xff9e64)
    }
    pub(crate) fn done() -> Rgba {
        rgb(0x9ece6a)
    }
    pub(crate) fn failed() -> Rgba {
        rgb(0xf7768e)
    }
}

/// The icon and colour of an agent's state, as the TUI's sidebar shows them.
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

/// The xterm 256-colour palette entry for an index, from the 16 ANSI colours above.
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

/// What `cornercase theme -` reads.
pub(crate) fn theme_json() -> String {
    let palette: Vec<[u8; 3]> = (0..=255u8).map(palette).collect();
    json!({ "foreground": FOREGROUND, "background": BACKGROUND, "palette": palette }).to_string()
}

pub(crate) fn hsla([r, g, b]: [u8; 3]) -> Hsla {
    Rgba { r: f32::from(r) / 255., g: f32::from(g) / 255., b: f32::from(b) / 255., a: 1. }.into()
}

/// Resolves colours the way a pane's program left them: its own overrides first.
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

    /// The foreground and background a cell of this style is drawn with, and whether its
    /// background differs from the pane's.
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
