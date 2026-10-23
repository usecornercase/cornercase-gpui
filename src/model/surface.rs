use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Colour {
    Index(u8),
    Rgb([u8; 3]),
}

#[expect(clippy::struct_excessive_bools, reason = "mirrors the attributes of cornercase's style table")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Style {
    pub fg: Option<Colour>,
    pub bg: Option<Colour>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    #[serde(rename = "underline_style")]
    pub underline_kind: Option<UnderlineKind>,
    pub underline_colour: Option<Colour>,
    pub overline: bool,
    pub blink: bool,
    pub inverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum UnderlineKind {
    Double,
    Curly,
    Dotted,
    Dashed,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Hyperlink {
    pub uri: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Area {
    pub x: u16,
    pub y: u16,
    pub cols: u16,
    pub rows: u16,
}

impl Area {
    pub(crate) fn contains(self, x: u16, y: u16) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.cols && y < self.y + self.rows
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Size {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Divider {
    pub dir: String,
    pub path: Vec<bool>,
    pub area: Area,
    pub line: Area,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CursorShape {
    #[default]
    Block,
    Bar,
    Underline,
    BlockHollow,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Cursor {
    pub x: u16,
    pub y: u16,
    pub shape: CursorShape,
    pub blinking: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MouseMode {
    #[default]
    None,
    Press,
    PressRelease,
    ButtonMotion,
    AnyMotion,
}

#[expect(clippy::struct_excessive_bools, reason = "mirrors the terminal modes cornercase reports")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Modes {
    pub mouse: MouseMode,
    pub alternate_screen: bool,
    pub alternate_scroll: bool,
    pub bracketed_paste: bool,
    pub application_cursor: bool,
    pub focus_events: bool,
    pub kitty_keyboard: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Scroll {
    pub total: u64,
    pub offset: u64,
    pub rows: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Colours {
    pub fg: Option<[u8; 3]>,
    pub bg: Option<[u8; 3]>,
    pub cursor: Option<[u8; 3]>,
    pub palette: BTreeMap<String, [u8; 3]>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct WireRun {
    pub style: usize,
    pub hyperlink: Option<usize>,
    pub cells: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum WireLines {
    All(Vec<Vec<WireRun>>),
    Changed(HashMap<String, Vec<WireRun>>),
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct Copy {
    pub from: u16,
    pub to: u16,
    pub count: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WirePane {
    pub pane: u64,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub rect: Option<Area>,
    #[serde(default)]
    pub size: Option<Size>,
    #[serde(default)]
    pub cursor: Option<Cursor>,
    #[serde(default)]
    pub modes: Option<Modes>,
    #[serde(default)]
    pub scroll: Option<Scroll>,
    #[serde(default)]
    pub colours: Option<Colours>,
    #[serde(default)]
    pub copy: Option<Copy>,
    #[serde(default)]
    pub lines: Option<WireLines>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Frame {
    pub boot: String,
    pub revision: u64,
    pub surface: u64,
    pub tab: u64,
    pub cols: u16,
    pub rows: u16,
    pub base: Option<u64>,
    pub projection: Option<u64>,
    pub dividers: Vec<Divider>,
    pub styles: Vec<Style>,
    pub hyperlinks: Vec<Hyperlink>,
    pub panes: Vec<WirePane>,
}
