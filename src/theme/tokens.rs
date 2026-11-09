use gpui::Rgba;
use serde::{Deserialize, Serialize};

use super::colour::{from_bytes, mix};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Group {
    Ground,
    Text,
    Lines,
    Accent,
    Status,
}

impl Group {
    pub(crate) const ALL: [Group; 5] = [Group::Ground, Group::Text, Group::Lines, Group::Accent, Group::Status];

    pub(crate) fn caption(self) -> &'static str {
        match self {
            Group::Ground => "Ground and surfaces",
            Group::Text => "Text",
            Group::Lines => "Lines and states",
            Group::Accent => "Accent",
            Group::Status => "Status",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Token {
    pub key: &'static str,
    pub group: Group,
    pub label: &'static str,
    pub note: &'static str,
    pub contrast: Option<f32>,
    pub derived: bool,
}

macro_rules! tokens {
    ($($key:ident, $group:ident, $label:literal, $note:literal, $need:expr, $derived:literal;)*) => {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub(crate) struct Chrome {
            $(pub $key: Rgba,)*
        }

        impl Chrome {
            pub(crate) fn blank() -> Chrome {
                let black = Rgba { r: 0., g: 0., b: 0., a: 1. };
                Chrome { $($key: black,)* }
            }

            pub(crate) fn get(&self, key: &str) -> Option<Rgba> {
                match key {
                    $(stringify!($key) => Some(self.$key),)*
                    _ => None,
                }
            }

            pub(crate) fn set(&mut self, key: &str, colour: Rgba) -> bool {
                match key {
                    $(stringify!($key) => self.$key = colour,)*
                    _ => return false,
                }
                true
            }
        }

        pub(crate) const TOKENS: &[Token] = &[
            $(Token {
                key: stringify!($key),
                group: Group::$group,
                label: $label,
                note: $note,
                contrast: $need,
                derived: $derived,
            },)*
        ];
    };
}

tokens! {
    glass, Ground, "Ground", "The window's glass, behind everything. How see-through it is sits under Transparency.", None, false;
    surface, Ground, "Cards", "The cards that hold panels and the sidebar.", None, false;
    panel, Ground, "Panel wash", "A light wash on panels.", None, false;
    raised, Ground, "Raised", "Menus, popovers and dialogs.", None, false;
    field, Ground, "Fields", "The inside of text fields.", None, false;
    veil, Ground, "Dimming", "What dims the window behind a sheet.", None, false;
    inset, Ground, "Inset", "Chips and table headers.", None, false;
    text, Text, "Text", "Titles and body text.", Some(4.5), false;
    soft, Text, "Soft text", "Secondary text.", Some(4.5), false;
    muted, Text, "Muted text", "Captions and hints.", Some(3.), false;
    faint, Text, "Faint text", "Placeholders and the quietest labels.", Some(3.), false;
    border, Lines, "Border", "Lines between things.", None, false;
    edge, Lines, "Edge", "Lines that must stand out.", None, false;
    hover, Lines, "Hover", "What a row takes under the pointer.", None, false;
    selected, Lines, "Selected", "What a chosen row takes.", None, false;
    brand, Accent, "Brand", "Primary buttons and the cobalt of the mark.", None, false;
    brand_hover, Accent, "Brand hovered", "A primary button under the pointer.", None, true;
    on_brand, Accent, "On brand", "Text and icons over the brand colour.", None, false;
    accent, Accent, "Accent", "Links and accent text.", None, false;
    accent_soft, Accent, "Accent wash", "A soft background for accent chips.", None, true;
    selection, Accent, "Selection", "Selected text in fields.", None, true;
    working, Status, "Working", "An agent at work.", None, false;
    waiting, Status, "Waiting", "An agent that waits for you.", None, false;
    done, Status, "Done", "A finished agent.", None, false;
    failed, Status, "Failed", "An error.", None, false;
    merged, Status, "Merged", "A merged pull request.", None, false;
}

pub(crate) fn derive(chrome: &Chrome, key: &str) -> Option<Rgba> {
    let black = from_bytes([0, 0, 0]);
    match key {
        "brand_hover" => Some(mix(black, chrome.brand, 0.847)),
        "accent_soft" => Some(Rgba { a: 46. / 255., ..chrome.brand }),
        "selection" => Some(Rgba { a: 71. / 255., ..chrome.brand }),
        _ => None,
    }
}

pub(crate) const OPAQUE: [&str; 3] = ["glass", "surface", "veil"];

pub(crate) const TERMINAL_KEYS: [&str; 19] = [
    "foreground",
    "background",
    "cursor",
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
];

pub(crate) const ANSI_NAMES: &[&str] = &[
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Opacity {
    pub ground: f32,
    pub panes: f32,
    pub cards: f32,
    pub floating: f32,
    pub veil: f32,
}

impl Opacity {
    pub(crate) const KEYS: [&'static str; 5] = ["ground", "panes", "cards", "floating", "veil"];

    pub(crate) const LIMITS: [(f32, f32); 5] = [(0.55, 1.), (0.3, 1.), (0., 0.8), (0.8, 1.), (0., 0.9)];

    pub(crate) fn get(&self, key: &str) -> Option<f32> {
        match key {
            "ground" => Some(self.ground),
            "panes" => Some(self.panes),
            "cards" => Some(self.cards),
            "floating" => Some(self.floating),
            "veil" => Some(self.veil),
            _ => None,
        }
    }

    pub(crate) fn set(&mut self, key: &str, value: f32) -> bool {
        let Some(at) = Self::KEYS.iter().position(|k| *k == key) else { return false };
        let (least, most) = Self::LIMITS[at];
        let value = value.clamp(least, most);
        match key {
            "ground" => self.ground = value,
            "panes" => self.panes = value,
            "cards" => self.cards = value,
            "floating" => self.floating = value,
            _ => self.veil = value,
        }
        true
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Radius {
    Compact,
    Round,
    #[default]
    #[serde(other)]
    Default,
}

impl Radius {
    pub(crate) const ALL: [Radius; 3] = [Radius::Compact, Radius::Default, Radius::Round];

    pub(crate) fn key(self) -> &'static str {
        match self {
            Radius::Compact => "compact",
            Radius::Default => "default",
            Radius::Round => "round",
        }
    }

    pub(crate) fn steps(self) -> [f32; 4] {
        match self {
            Radius::Compact => [3., 5., 8., 10.],
            Radius::Default => [5., 8., 12., 16.],
            Radius::Round => [7., 11., 16., 22.],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shape {
    pub radius: Radius,
    pub glow: bool,
}

#[cfg(test)]
mod tests;
