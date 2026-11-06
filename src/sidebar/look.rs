use serde::Deserialize;
use serde_json::Value;

use crate::app::Root;
use crate::model::accounts::pick::AccountSettings;
use crate::store::{self, ToastKind};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Arrangement {
    SideBySide,
    ProjectsOnTop,
    WorkspacesOnTop,
    #[default]
    Tree,
}

#[expect(clippy::struct_excessive_bools, reason = "mirrors settings of config --json that are switches")]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Look {
    pub arrangement: Arrangement,
    pub tabs_in_sidebar: bool,
    pub agents_section: bool,
    pub account_mark: bool,
    pub account_switcher: bool,
    pub account_picker: bool,
    pub default_agent: Option<String>,
    pub boot: String,
    pub revision: u64,
}

#[derive(Deserialize)]
struct Described {
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    settings: Vec<Setting>,
}

#[derive(Deserialize)]
struct Setting {
    key: String,
    #[serde(default)]
    value: Value,
}

fn arrangement(value: &str) -> Arrangement {
    match value {
        "side_by_side" => Arrangement::SideBySide,
        "projects_on_top" => Arrangement::ProjectsOnTop,
        "workspaces_on_top" => Arrangement::WorkspacesOnTop,
        _ => Arrangement::Tree,
    }
}

fn named_agent(agent: &str) -> Option<String> {
    let agent = agent.trim();
    (!agent.is_empty() && agent != "auto").then(|| agent.to_string())
}

pub(crate) fn parse(text: &str) -> anyhow::Result<Look> {
    let described: Described = serde_json::from_str(text)?;
    let value = |key: &str| described.settings.iter().find(|s| s.key == key).map(|s| &s.value);
    Ok(Look {
        arrangement: value("sidebar").and_then(Value::as_str).map_or(Arrangement::Tree, arrangement),
        tabs_in_sidebar: value("tabs").and_then(Value::as_str) == Some("sidebar"),
        agents_section: value("agents_section").and_then(Value::as_bool).unwrap_or(false),
        account_mark: value("account_mark").and_then(Value::as_bool).unwrap_or(true),
        account_switcher: value("account_switcher").and_then(Value::as_bool).unwrap_or(true),
        account_picker: value("account_picker").and_then(Value::as_bool).unwrap_or(true),
        default_agent: value("agent").and_then(Value::as_str).and_then(named_agent),
        boot: String::new(),
        revision: described.revision,
    })
}

impl Look {
    fn account_settings(&self) -> AccountSettings {
        AccountSettings {
            mark: self.account_mark,
            picker: self.account_picker,
            default_agent: self.default_agent.clone(),
        }
    }
}

#[derive(Default)]
pub(crate) struct Looks {
    pub current: Look,
    loading: bool,
}

impl Root {
    pub(crate) fn follow_look(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(session) = &self.session else { return };
        let look = &self.side.look.current;
        let stale = session.boot != look.boot || session.config.revision > look.revision;
        if !stale || self.side.look.loading {
            return;
        }
        self.side.look.loading = true;
        let boot = session.boot.clone();
        store::run(cx, store::args(&["config", "--json"]), move |this: &mut Root, result, cx| {
            this.side.look.loading = false;
            match result.and_then(|out| parse(&out)) {
                Ok(look) => this.take_look(Look { boot, ..look }, cx),
                Err(e) => {
                    this.side.look.current.boot = boot;
                    let text = format!("Could not read the sidebar's settings: {e:#}");
                    this.store.update(cx, |s, cx| s.toast(text, ToastKind::Error, cx));
                }
            }
            cx.notify();
        });
    }

    fn take_look(&mut self, look: Look, cx: &mut gpui::Context<Self>) {
        let settings = look.account_settings();
        self.store.update(cx, |s, cx| s.take_account_settings(settings, cx));
        self.side.look.current = look;
    }
}

#[cfg(test)]
mod tests;
