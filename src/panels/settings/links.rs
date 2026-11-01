use gpui::{AnyElement, Context, IntoElement, ParentElement as _, Window};
use serde_json::Value;

use super::data::{self, Choice, Described, Kind, LINKS, Setting};
use super::pages::page_icon;
use super::{Role, SettingsPanel, widgets};
use crate::prefs::Prefs;
use crate::store::Store;

const PREFIX: &str = "app.";

struct Own {
    key: &'static str,
    label: &'static str,
    note: &'static str,
    choices: &'static [(&'static str, &'static str)],
}

const OWN: [Own; 2] = [
    Own {
        key: "open_links",
        label: "web addresses open in",
        note: "Where a click on a web address in the terminal opens it. ⌘-click opens it in your default browser \
               either way. An address of another machine's own server (localhost) opens in the Browser panel, \
               which reaches it over ssh.",
        choices: &[
            ("default_browser", "The browser macOS opens links with"),
            ("browser_panel", "The Browser panel, beside the terminal"),
        ],
    },
    Own {
        key: "preview_pages",
        label: "page previews",
        note: "Which pages show in the card that opens when the pointer rests on a web address. Loading a page can \
               act on it, such as using up a one-time sign-in link, so the others load when you ask. Previews \
               load without your cookies and never download.",
        choices: &[
            ("local_addresses", "Pages of a server on this machine, or on the one shown (localhost), load at once"),
            ("every_address", "Every page loads at once"),
            ("when_asked", "No page loads until you ask"),
        ],
    },
];

pub(super) fn owned(key: &str) -> Option<&str> {
    key.strip_prefix(PREFIX).filter(|own| OWN.iter().any(|o| o.key == *own))
}

fn setting(own: &Own, values: &serde_json::Map<String, Value>, defaults: &serde_json::Map<String, Value>) -> Setting {
    let choices = own.choices.iter().map(|(value, note)| Choice {
        value: (*value).into(),
        note: (*note).into(),
        dangerous: false,
    });
    Setting {
        key: format!("{PREFIX}{}", own.key),
        page: LINKS.into(),
        label: own.label.into(),
        kind: Kind::Choice,
        value: values.get(own.key).cloned().unwrap_or_default(),
        default: defaults.get(own.key).cloned().unwrap_or_default(),
        note: own.note.into(),
        choices: choices.collect(),
        ..Setting::default()
    }
}

pub(super) fn settings(prefs: &Prefs) -> Vec<Setting> {
    let (values, defaults) = (prefs.values(), Prefs::default().values());
    OWN.iter().map(|own| setting(own, &values, &defaults)).collect()
}

pub(super) fn add(described: &mut Described, prefs: &Prefs) {
    described.settings.retain(|s| s.page != LINKS);
    described.settings.extend(settings(prefs));
}

pub(super) fn alone(prefs: &Prefs) -> Described {
    let mut described = Described::default();
    add(&mut described, prefs);
    described
}

impl SettingsPanel {
    pub(super) fn own_page(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let own = alone(&Store::get(cx).read(cx).prefs);
        widgets::column()
            .child(widgets::heading(page_icon(LINKS), LINKS.to_string(), data::about(LINKS)))
            .children(self.settings_sections(&own, LINKS, window, cx))
            .into_any_element()
    }

    pub(super) fn save_own(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        let Some(own) = owned(key) else { return };
        let role = Role::Setting(key.to_string());
        let store = Store::get(cx);
        match store.update(cx, |s, cx| s.set_pref(own, value, cx)) {
            Ok(()) => {
                self.errors.remove(&role);
                let prefs = store.read(cx).prefs.clone();
                if let Some(described) = self.described.as_mut() {
                    add(described, &prefs);
                }
            }
            Err(e) => {
                self.errors.insert(role, format!("{e:#}"));
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
