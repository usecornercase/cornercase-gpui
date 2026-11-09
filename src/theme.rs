mod fonts;

pub(crate) mod colour;
pub(crate) mod current;
pub(crate) mod file;
pub(crate) mod presets;
pub(crate) mod resolve;
pub(crate) mod tokens;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{Hsla, Rgba};

use crate::model::{Colour, Colours, Style};
use crate::platform::Os;

use colour::{from_bytes, opaque};

pub(crate) use current::current;
pub(crate) use fonts::{FONT_FAMILY, UI_FAMILY, missing as missing_font, settle as settle_fonts};
pub(crate) use resolve::Theme;

pub(crate) const INACTIVE: f32 = 0.55;

static BACKDROP: AtomicBool = AtomicBool::new(false);

pub(crate) mod radius {
    use super::current::read;

    pub(crate) fn chip() -> f32 {
        read(|t| t.shape.radius.steps()[0])
    }
    pub(crate) fn control() -> f32 {
        read(|t| t.shape.radius.steps()[1])
    }
    pub(crate) fn card() -> f32 {
        read(|t| t.shape.radius.steps()[2])
    }
    pub(crate) fn sheet() -> f32 {
        read(|t| t.shape.radius.steps()[3])
    }
}

pub(crate) mod colours {
    use super::colour::{from_bytes, mix, opaque};
    use super::current::read;
    use super::{Rgba, ground, over_ground, terminal_opacity};

    macro_rules! plain {
        ($($name:ident),* $(,)?) => {
            $(pub(crate) fn $name() -> Rgba {
                read(|t| t.chrome.$name)
            })*
        };
    }

    plain!(
        brand,
        brand_hover,
        on_brand,
        accent,
        accent_soft,
        selection,
        panel,
        raised,
        field,
        border,
        edge,
        hover,
        selected,
        text,
        soft,
        muted,
        faint,
        working,
        waiting,
        done,
        failed,
        merged
    );

    pub(crate) fn inset() -> Rgba {
        read(|t| t.chrome.inset)
    }

    pub(crate) fn brand_light() -> Rgba {
        mix(brand(), from_bytes([255, 255, 255]), 0.17)
    }

    pub(crate) fn glass() -> Rgba {
        read(|t| ground(t, super::Os::CURRENT, super::BACKDROP.load(super::Ordering::Relaxed)))
    }

    pub(crate) fn veil() -> Rgba {
        read(|t| Rgba { a: t.opacity.veil, ..t.chrome.veil })
    }

    pub(crate) fn surface() -> Rgba {
        read(|t| Rgba { a: t.opacity.cards, ..t.chrome.surface })
    }

    pub(crate) fn raised_hover() -> Rgba {
        read(|t| opaque(t.chrome.surface))
    }

    pub(crate) fn ink() -> Rgba {
        read(|t| opaque(t.chrome.glass))
    }

    pub(crate) fn terminal() -> Rgba {
        read(|t| Rgba { a: terminal_opacity(), ..from_bytes(t.terminal.background) })
    }

    pub(crate) fn pane() -> Rgba {
        over_ground(terminal(), glass())
    }

    pub(crate) fn card_top() -> Rgba {
        read(|t| Rgba { a: t.opacity.floating, ..mix(t.chrome.raised, opaque(t.chrome.glass), 0.05) })
    }

    pub(crate) fn card_bottom() -> Rgba {
        read(|t| Rgba { a: (t.opacity.floating + 0.03).min(1.), ..mix(t.chrome.raised, opaque(t.chrome.glass), 0.7) })
    }

    pub(crate) fn sunk() -> Rgba {
        read(|t| mix(t.chrome.raised, opaque(t.chrome.glass), 0.35))
    }

    pub(crate) fn white() -> Rgba {
        from_bytes([255, 255, 255])
    }

    pub(crate) fn caption_close() -> Rgba {
        from_bytes([0xc4, 0x2b, 0x1c])
    }
    pub(crate) fn on_caption_close() -> Rgba {
        from_bytes([0xff, 0xff, 0xff])
    }
    pub(crate) fn code_ground() -> Rgba {
        from_bytes([0xff, 0xff, 0xff])
    }
    pub(crate) fn code_ink() -> Rgba {
        from_bytes([0, 0, 0])
    }
}

pub(crate) fn shade(alpha: f32) -> Hsla {
    let softer = current::read(Theme::is_light);
    Hsla { h: 0., s: 0., l: 0., a: if softer { alpha * 0.45 } else { alpha } }
}

pub(crate) fn palette(index: u8) -> [u8; 3] {
    current::read(|t| t.terminal.palette(index))
}

pub(crate) fn ansi(index: u8) -> Hsla {
    hsla(palette(index))
}

pub(crate) fn palette_rgba(index: u8) -> Rgba {
    from_bytes(palette(index))
}

pub(crate) fn terminal_opacity() -> f32 {
    current::opacity_override().unwrap_or_else(|| current::read(|t| t.opacity.panes))
}

pub(crate) fn backdrop_shown(shown: bool) {
    BACKDROP.store(shown, Ordering::Relaxed);
}

pub(crate) fn ground(theme: &Theme, os: Os, backdrop: bool) -> Rgba {
    match (os, backdrop) {
        (Os::Mac, _) | (Os::Windows, true) => Rgba { a: theme.opacity.ground, ..theme.chrome.glass },
        (Os::Linux | Os::Windows, _) => opaque(theme.chrome.glass),
    }
}

pub(crate) fn translucent_ground() -> bool {
    colours::glass().a < 1.
}

pub(crate) fn over_ground(wanted: Rgba, ground: Rgba) -> Rgba {
    if ground.a >= 1. {
        return wanted;
    }
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

pub(crate) fn fades(dim_inactive: bool, several: bool, active: bool) -> bool {
    dim_inactive && several && !active
}

pub(crate) fn terminal_json() -> String {
    current::read(|t| t.terminal.json())
}

pub(crate) fn hsla([r, g, b]: [u8; 3]) -> Hsla {
    Rgba { r: f32::from(r) / 255., g: f32::from(g) / 255., b: f32::from(b) / 255., a: 1. }.into()
}

pub(crate) struct Painter<'a> {
    pub colours: &'a Colours,
    pub theme: Arc<Theme>,
}

impl<'a> Painter<'a> {
    pub(crate) fn new(colours: &'a Colours) -> Self {
        Painter { colours, theme: current() }
    }

    pub(crate) fn default_fg(&self) -> [u8; 3] {
        self.colours.fg.unwrap_or(self.theme.terminal.foreground)
    }

    pub(crate) fn default_bg(&self) -> [u8; 3] {
        self.colours.bg.unwrap_or(self.theme.terminal.background)
    }

    fn colour(&self, colour: Colour) -> [u8; 3] {
        match colour {
            Colour::Rgb(rgb) => rgb,
            Colour::Index(i) => {
                self.colours.palette.get(&i.to_string()).copied().unwrap_or_else(|| self.theme.terminal.palette(i))
            }
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
        (bg != self.theme.terminal.background).then(|| hsla(bg))
    }

    pub(crate) fn cell_fill(&self, bg: Hsla, custom: bool) -> Option<Hsla> {
        (custom && bg != hsla(self.default_bg())).then_some(bg)
    }

    pub(crate) fn underline_colour(&self, style: &Style) -> Option<Hsla> {
        style.underline_colour.map(|c| hsla(self.colour(c)))
    }

    pub(crate) fn cursor(&self) -> Hsla {
        hsla(self.colours.cursor.unwrap_or_else(|| self.colours.fg.unwrap_or(self.theme.terminal.cursor)))
    }
}

#[cfg(test)]
mod tests;
