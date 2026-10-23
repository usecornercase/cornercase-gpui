//! gpui keystrokes to `cornercase input` events.
//!
//! Printable text is left to the platform's input method (it composes accents, and ⌥2 types `@`
//! on a Spanish keyboard), which hands it to our input handler. Here we only take the keys that
//! are not text: named keys, and anything with ctrl.

use gpui::Keystroke;
use serde_json::{Value, json};

const NAMED: &[&str] = &[
    "enter", "escape", "tab", "backspace", "up", "down", "left", "right", "home", "end", "pageup", "pagedown",
    "delete", "insert", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12",
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

/// The event for a key, or `None` when it is text for the input method to type.
pub fn key_event(keystroke: &Keystroke) -> Option<Value> {
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
        // ctrl+space, ctrl+letter, ctrl+[ …: the letter itself, lower case.
        let code = if key == "space" { "space" } else { code };
        return event(code);
    }
    if key == "space" && (m.alt || m.shift) {
        return event("space");
    }
    // ⌥ with a key that types nothing else is meta (⌥← moves by words in the shell).
    if m.alt && keystroke.key_char.as_deref().is_none_or(|c| c == key) {
        return event(code);
    }
    None
}
