use serde::Deserialize;
use serde_json::Value;

use crate::app::Root;
use crate::model::PaneInfo;
use crate::model::accounts::pick::AccountSettings;
use crate::store::{self, ToastKind};
use crate::theme::fades;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Arrangement {
    SideBySide,
    ProjectsOnTop,
    WorkspacesOnTop,
    #[default]
    Tree,
}

#[expect(clippy::struct_excessive_bools, reason = "mirrors settings of config --json that are switches")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Look {
    pub arrangement: Arrangement,
    pub tabs_in_sidebar: bool,
    pub agents_section: bool,
    pub account_mark: bool,
    pub account_switcher: bool,
    pub model: bool,
    pub context: bool,
    pub counts: bool,
    pub todo_dot: bool,
    pub dim_inactive_panes: bool,
    pub account_picker: bool,
    pub default_agent: Option<String>,
    pub setups: Vec<String>,
    pub boot: String,
    pub revision: u64,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            arrangement: Arrangement::default(),
            tabs_in_sidebar: false,
            agents_section: false,
            account_mark: false,
            account_switcher: false,
            account_picker: false,
            default_agent: None,
            setups: Vec::new(),
            model: true,
            context: true,
            counts: true,
            todo_dot: true,
            dim_inactive_panes: true,
            boot: String::new(),
            revision: 0,
        }
    }
}

impl Look {
    pub(crate) fn model_of<Id>(&self, pane: &PaneInfo<Id>) -> Option<String> {
        pane.model.clone().filter(|_| self.model)
    }

    pub(crate) fn share_of<Id>(&self, pane: &PaneInfo<Id>) -> Option<String> {
        let share = pane.context.map(|c| format!("{c}%")).or_else(|| pane.compacted.then(|| "compacted".to_string()));
        share.filter(|_| self.context)
    }

    pub(crate) fn fades(&self, several: bool, active: bool) -> bool {
        fades(self.dim_inactive_panes, several, active)
    }
}

#[derive(Deserialize)]
struct Described {
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    settings: Vec<Setting>,
    #[serde(default)]
    projects: Vec<ProjectScripts>,
}

#[derive(Deserialize)]
struct ProjectScripts {
    folder: String,
    #[serde(default)]
    setup: String,
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
    let on = |key: &str| value(key).and_then(Value::as_bool).unwrap_or(true);
    Ok(Look {
        arrangement: value("sidebar").and_then(Value::as_str).map_or(Arrangement::Tree, arrangement),
        tabs_in_sidebar: value("tabs").and_then(Value::as_str) == Some("sidebar"),
        agents_section: value("agents_section").and_then(Value::as_bool).unwrap_or(false),
        account_mark: on("account_mark"),
        account_switcher: on("account_switcher"),
        account_picker: on("account_picker"),
        default_agent: value("agent").and_then(Value::as_str).and_then(named_agent),
        setups: described.projects.iter().filter(|p| !p.setup.trim().is_empty()).map(|p| p.folder.clone()).collect(),
        model: on("model"),
        context: on("context"),
        counts: on("counts"),
        todo_dot: on("todo_dot"),
        dim_inactive_panes: on("dim_inactive_panes"),
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
