use gpui::{Keystroke, Modifiers};
use serde_json::{Map, Value, json};

const NAMED: &[(&str, &str, u16)] = &[
    ("enter", "Enter", 13),
    ("escape", "Escape", 27),
    ("tab", "Tab", 9),
    ("backspace", "Backspace", 8),
    ("up", "ArrowUp", 38),
    ("down", "ArrowDown", 40),
    ("left", "ArrowLeft", 37),
    ("right", "ArrowRight", 39),
    ("home", "Home", 36),
    ("end", "End", 35),
    ("pageup", "PageUp", 33),
    ("pagedown", "PageDown", 34),
    ("delete", "Delete", 46),
    ("insert", "Insert", 45),
];

const PUNCTUATION: &[(&str, &str, u16)] = &[
    ("`~", "Backquote", 192),
    ("-_", "Minus", 189),
    ("=+", "Equal", 187),
    ("[{", "BracketLeft", 219),
    ("]}", "BracketRight", 221),
    ("\\|", "Backslash", 220),
    (";:", "Semicolon", 186),
    ("'\"", "Quote", 222),
    (",<", "Comma", 188),
    (".>", "Period", 190),
    ("/?", "Slash", 191),
];

const SHIFTED_DIGITS: &str = ")!@#$%^&*(";

const COMMANDS: &[(&str, &str)] = &[
    ("backspace", "deleteBackward"),
    ("shift-backspace", "deleteBackward"),
    ("alt-backspace", "deleteWordBackward"),
    ("alt-shift-backspace", "deleteWordBackward"),
    ("cmd-backspace", "deleteToBeginningOfLine"),
    ("shift-cmd-backspace", "deleteToBeginningOfLine"),
    ("delete", "deleteForward"),
    ("shift-delete", "deleteForward"),
    ("alt-delete", "deleteWordForward"),
    ("up", "moveUp"),
    ("down", "moveDown"),
    ("left", "moveLeft"),
    ("right", "moveRight"),
    ("shift-up", "moveUpAndModifySelection"),
    ("shift-down", "moveDownAndModifySelection"),
    ("shift-left", "moveLeftAndModifySelection"),
    ("shift-right", "moveRightAndModifySelection"),
    ("alt-left", "moveWordLeft"),
    ("alt-right", "moveWordRight"),
    ("alt-up", "moveBackward moveToBeginningOfParagraph"),
    ("alt-down", "moveForward moveToEndOfParagraph"),
    ("alt-shift-left", "moveWordLeftAndModifySelection"),
    ("alt-shift-right", "moveWordRightAndModifySelection"),
    ("alt-shift-up", "moveParagraphBackwardAndModifySelection"),
    ("alt-shift-down", "moveParagraphForwardAndModifySelection"),
    ("cmd-left", "moveToLeftEndOfLine"),
    ("cmd-right", "moveToRightEndOfLine"),
    ("cmd-up", "moveToBeginningOfDocument"),
    ("cmd-down", "moveToEndOfDocument"),
    ("shift-cmd-left", "moveToLeftEndOfLineAndModifySelection"),
    ("shift-cmd-right", "moveToRightEndOfLineAndModifySelection"),
    ("shift-cmd-up", "moveToBeginningOfDocumentAndModifySelection"),
    ("shift-cmd-down", "moveToEndOfDocumentAndModifySelection"),
    ("home", "scrollToBeginningOfDocument"),
    ("end", "scrollToEndOfDocument"),
    ("shift-home", "moveToBeginningOfDocumentAndModifySelection"),
    ("shift-end", "moveToEndOfDocumentAndModifySelection"),
    ("pageup", "scrollPageUp"),
    ("pagedown", "scrollPageDown"),
    ("shift-pageup", "pageUpAndModifySelection"),
    ("shift-pagedown", "pageDownAndModifySelection"),
    ("ctrl-a", "moveToBeginningOfParagraph"),
    ("ctrl-b", "moveBackward"),
    ("ctrl-d", "deleteForward"),
    ("ctrl-e", "moveToEndOfParagraph"),
    ("ctrl-f", "moveForward"),
    ("ctrl-h", "deleteBackward"),
    ("ctrl-k", "deleteToEndOfParagraph"),
    ("ctrl-n", "moveDown"),
    ("ctrl-p", "moveUp"),
    ("ctrl-t", "transpose"),
    ("ctrl-shift-a", "moveToBeginningOfParagraphAndModifySelection"),
    ("ctrl-shift-e", "moveToEndOfParagraphAndModifySelection"),
    ("cmd-a", "selectAll"),
    ("cmd-z", "undo"),
    ("shift-cmd-z", "redo"),
];

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Route {
    Key,
    Text,
    Repeat(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Key {
    pub(crate) name: String,
    pub(crate) code: String,
    pub(crate) vk: u16,
}

pub(crate) fn modifiers(m: Modifiers) -> u8 {
    u8::from(m.alt) | u8::from(m.control) << 1 | u8::from(m.platform) << 2 | u8::from(m.shift) << 3
}

fn named(key: &str) -> Option<Key> {
    if let Some(&(_, dom, vk)) = NAMED.iter().find(|(name, ..)| *name == key) {
        return Some(Key { name: dom.to_string(), code: dom.to_string(), vk });
    }
    let n: u16 = key.strip_prefix('f')?.parse().ok().filter(|n| (1..=24).contains(n))?;
    let dom = format!("F{n}");
    Some(Key { name: dom.clone(), code: dom, vk: 111 + n })
}

fn physical(c: char) -> Option<(String, u16)> {
    if c.is_ascii_alphabetic() {
        let up = c.to_ascii_uppercase();
        return Some((format!("Key{up}"), u16::from(up as u8)));
    }
    if let Some(d) = c.to_digit(10).or_else(|| SHIFTED_DIGITS.find(c).and_then(|i| u32::try_from(i).ok())) {
        return Some((format!("Digit{d}"), 48 + u16::try_from(d).ok()?));
    }
    if c == ' ' {
        return Some(("Space".to_string(), 32));
    }
    PUNCTUATION.iter().find(|(chars, ..)| chars.contains(c)).map(|&(_, code, vk)| (code.to_string(), vk))
}

fn printed(ks: &Keystroke) -> String {
    let typed = ks.key_char.as_deref().filter(|t| !t.is_empty() && !t.chars().any(char::is_control));
    match typed {
        Some(text) => text.to_string(),
        None if ks.key == "space" => " ".to_string(),
        None if ks.modifiers.shift && ks.key.len() == 1 => ks.key.to_uppercase(),
        None => ks.key.clone(),
    }
}

pub(crate) fn describe(ks: &Keystroke) -> Key {
    if let Some(key) = named(&ks.key) {
        return key;
    }
    let base = if ks.key == "space" { Some(' ') } else { single(&ks.key) };
    let (code, vk) = base.and_then(physical).unwrap_or_default();
    Key { name: printed(ks), code, vk }
}

fn single(text: &str) -> Option<char> {
    let mut chars = text.chars();
    chars.next().filter(|_| chars.next().is_none())
}

fn chord(ks: &Keystroke) -> String {
    let m = ks.modifiers;
    let held = [("ctrl-", m.control), ("alt-", m.alt), ("shift-", m.shift), ("cmd-", m.platform)];
    let mut chord: String = held.iter().filter(|(_, on)| *on).map(|(name, _)| *name).collect();
    chord.push_str(&ks.key);
    chord
}

pub(crate) fn commands(ks: &Keystroke) -> Vec<&'static str> {
    let chord = chord(ks);
    COMMANDS.iter().find(|(c, _)| *c == chord).map(|(_, names)| names.split(' ').collect()).unwrap_or_default()
}

pub(crate) fn route(ks: &Keystroke, held: bool) -> Route {
    let m = ks.modifiers;
    if m.platform || m.control || m.function || named(&ks.key).is_some() {
        return Route::Key;
    }
    match ks.key_char.as_deref().filter(|t| !t.is_empty()) {
        Some(text) if held => Route::Repeat(text.to_string()),
        _ => Route::Text,
    }
}

fn event(kind: &str, ks: &Keystroke) -> Map<String, Value> {
    let key = describe(ks);
    let mut event = Map::new();
    event.insert("type".into(), kind.into());
    event.insert("key".into(), key.name.into());
    event.insert("code".into(), key.code.into());
    event.insert("windowsVirtualKeyCode".into(), key.vk.into());
    event.insert("modifiers".into(), modifiers(ks.modifiers).into());
    event
}

fn with_text(mut event: Map<String, Value>, text: &str) -> Map<String, Value> {
    event.insert("type".into(), "keyDown".into());
    event.insert("text".into(), text.into());
    event.insert("unmodifiedText".into(), text.into());
    event
}

pub(crate) fn key_down(ks: &Keystroke, held: bool) -> Value {
    let mut event = event("rawKeyDown", ks);
    if ks.key == "enter" && !ks.modifiers.platform {
        event = with_text(event, "\r");
    }
    event.insert("autoRepeat".into(), held.into());
    event.insert("commands".into(), json!(commands(ks)));
    Value::Object(event)
}

pub(crate) fn typed(ks: &Keystroke, text: &str, held: bool) -> Value {
    let mut event = with_text(event("keyDown", ks), text);
    event.insert("autoRepeat".into(), held.into());
    Value::Object(event)
}

pub(crate) fn key_up(ks: &Keystroke) -> Value {
    Value::Object(event("keyUp", ks))
}

#[cfg(test)]
mod tests;
