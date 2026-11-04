use gpui::Keystroke;
use serde_json::{Value, json};

use crate::platform::Os;

const NAMED: &[&str] = &[
    "enter",
    "escape",
    "tab",
    "backspace",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "pageup",
    "pagedown",
    "delete",
    "insert",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

fn mods(keystroke: &Keystroke) -> Vec<&'static str> {
    let m = keystroke.modifiers;
    let mut mods = Vec::new();
    if m.control {
        mods.push("ctrl");
    }
    if m.alt {
        mods.push("alt");
    }
    if m.shift {
        mods.push("shift");
    }
    mods
}

fn meta_text(keystroke: &Keystroke, os: Os) -> Option<&str> {
    let text = keystroke.key_char.as_deref().filter(|_| os != Os::Mac && keystroke.modifiers.alt)?;
    let mut chars = text.chars();
    chars.next().filter(|c| !c.is_control() && chars.next().is_none()).map(|_| text)
}

pub(crate) fn key_event(keystroke: &Keystroke, os: Os) -> Option<Value> {
    let key = keystroke.key.as_str();
    let m = keystroke.modifiers;
    let named = NAMED.contains(&key);
    let code = match key {
        "escape" => "esc",
        "space" => " ",
        _ => key,
    };
    let event = |code: &str| Some(json!({ "key": { "code": code, "mods": mods(keystroke) } }));

    if named {
        return event(code);
    }
    if m.control {
        let code = if key == "space" { "space" } else { code };
        return event(code);
    }
    if key == "space" && (m.alt || m.shift) {
        return event("space");
    }
    if m.alt && keystroke.key_char.as_deref().is_none_or(|c| c == key) {
        return event(code);
    }
    meta_text(keystroke, os).and_then(event)
}

#[cfg(test)]
mod tests;
