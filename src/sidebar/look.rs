use serde::Deserialize;
use serde_json::Value;

use crate::app::Root;
use crate::model::PaneInfo;
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
    pub(crate) fn model_of(&self, pane: &PaneInfo) -> Option<String> {
        pane.model.clone().filter(|_| self.model)
    }

    pub(crate) fn share_of(&self, pane: &PaneInfo) -> Option<String> {
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
        model: on("model"),
        context: on("context"),
        counts: on("counts"),
        todo_dot: on("todo_dot"),
        dim_inactive_panes: on("dim_inactive_panes"),
        boot: String::new(),
        revision: described.revision,
    })
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
                Ok(look) => this.side.look.current = Look { boot, ..look },
                Err(e) => {
                    this.side.look.current.boot = boot;
                    let text = format!("Could not read the sidebar's settings: {e:#}");
                    this.store.update(cx, |s, cx| s.toast(text, ToastKind::Error, cx));
                }
            }
            cx.notify();
        });
    }
}

#[cfg(test)]
mod tests;
