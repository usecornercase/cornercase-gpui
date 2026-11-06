use anyhow::Result;
use gpui::{App, Context};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::data::Ran;
use super::status::Status;
use super::{busy, job};
use crate::cli;
use crate::model::Shown;
use crate::store::{self, Store, ToastKind, args};

const POPUP_WAITS: &str = "a popup needs cornercase's own window open: needs usecornercase/cornercase-terminal#666";
pub(super) const VIEW_WAITS: &str = "A button's action runs on what cornercase's own window shows, and a popup needs \
                                     that window open: needs usecornercase/cornercase-terminal#666";
const BUTTON_WAITS: &str = "on what cornercase's own window shows, needs usecornercase/cornercase-terminal#666";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Spot {
    Project(u64),
    Workspace(u64),
    Tab(u64),
    Pane(u64),
    Button,
}

impl Spot {
    fn context(self) -> &'static str {
        match self {
            Self::Project(_) => "project",
            Self::Workspace(_) => "workspace",
            Self::Tab(_) => "tab",
            Self::Pane(_) => "pane",
            Self::Button => "button",
        }
    }

    fn args(self) -> Vec<String> {
        match self {
            Self::Project(id) | Self::Workspace(id) | Self::Tab(id) | Self::Pane(id) => {
                vec![format!("--{}", self.context()), id.to_string()]
            }
            Self::Button => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct ActionList {
    pub(super) actions: Vec<Action>,
    pub(super) revision: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Action {
    pub(super) plugin: String,
    #[serde(rename = "name")]
    pub(super) plugin_name: String,
    #[serde(rename = "action")]
    pub(super) id: String,
    pub(super) title: String,
    pub(super) contexts: Vec<String>,
    pub(super) placement: String,
}

impl Action {
    pub(super) fn key(&self) -> String {
        format!("{}/{}", self.plugin, self.id)
    }

    fn popup(&self) -> bool {
        self.placement == "popup"
    }

    pub(super) fn waits_on_the_view(&self) -> bool {
        self.popup() || self.offered_on(Spot::Button)
    }

    fn offered_on(&self, spot: Spot) -> bool {
        self.contexts.iter().any(|context| context == spot.context())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Run {
    action: Action,
    spot: Spot,
    revision: u64,
}

impl Run {
    pub(super) fn new(action: Action, spot: Spot, revision: u64) -> Self {
        Self { action, spot, revision }
    }

    pub(crate) fn title(&self) -> &str {
        &self.action.title
    }

    fn command(&self) -> Vec<String> {
        let (plugin, action, revision) = (&self.action.plugin, &self.action.id, self.revision.to_string());
        let mut command = args(&["plugin", "run", plugin, action, "--json", "--revision", &revision]);
        command.extend(self.spot.args());
        command
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Group {
    pub(crate) name: String,
    plugin: String,
    pub(crate) runs: Vec<Run>,
}

#[derive(Debug, Clone)]
pub(crate) struct Button {
    pub(crate) plugin: String,
    pub(crate) title: String,
    icon: String,
    pub(crate) runs: Vec<Run>,
}

impl Button {
    pub(crate) fn tip(&self) -> String {
        format!("{}: {BUTTON_WAITS}", self.title)
    }

    pub(crate) fn glyph(&self) -> String {
        if self.icon.chars().next().is_none_or(private_use) { initial(&self.title) } else { self.icon.clone() }
    }

    pub(crate) fn running(&self, cx: &App) -> bool {
        self.runs.iter().any(|run| busy(&run.action.key(), cx))
    }
}

fn private_use(c: char) -> bool {
    matches!(c, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{FFFFD}' | '\u{100000}'..='\u{10FFFD}')
}

fn initial(title: &str) -> String {
    title.chars().find(|c| !c.is_whitespace()).map_or_else(|| "?".into(), |c| c.to_uppercase().collect())
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Offers {
    pub(super) list: ActionList,
    pub(super) status: Option<Status>,
    asked: u64,
}

fn parsed<T: DeserializeOwned>(answer: Result<Value>) -> Result<T> {
    Ok(serde_json::from_value(answer?)?)
}

impl Offers {
    fn ask(&mut self) -> u64 {
        self.asked += 1;
        self.asked
    }

    fn read_actions(&mut self, asked: u64, answer: Result<Value>) -> Result<()> {
        if asked != self.asked {
            return Ok(());
        }
        self.list = ActionList::default();
        self.list = parsed(answer)?;
        Ok(())
    }

    fn read_status(&mut self, asked: u64, answer: Result<Value>) -> Result<()> {
        if asked != self.asked {
            return Ok(());
        }
        self.status = None;
        self.status = Some(parsed(answer)?);
        Ok(())
    }

    pub(crate) fn offered(&self, spot: Spot) -> Vec<Group> {
        let mut groups: Vec<Group> = Vec::new();
        for action in self.list.actions.iter().filter(|action| action.offered_on(spot)) {
            let run = Run::new(action.clone(), spot, self.list.revision);
            match groups.last_mut().filter(|group| group.plugin == action.plugin) {
                Some(group) => group.runs.push(run),
                None => groups.push(Group {
                    name: action.plugin_name.clone(),
                    plugin: action.plugin.clone(),
                    runs: vec![run],
                }),
            }
        }
        groups
    }

    pub(crate) fn buttons(&self) -> Vec<Button> {
        self.offered(Spot::Button).into_iter().map(|group| self.button(group)).collect()
    }

    fn button(&self, group: Group) -> Button {
        let face = self.status.as_ref().and_then(|status| status.face(&group.plugin));
        let title = face.map_or_else(|| group.name.clone(), |face| face.title.clone());
        let icon = face.map_or_else(|| initial(&title), |face| face.icon.clone());
        Button { plugin: group.plugin, title, icon, runs: group.runs }
    }
}

fn offers_read(store: &mut Store, read: Result<()>, cx: &mut Context<Store>) {
    if let Err(e) = read {
        store.toast(format!("Could not read the plugins: {}", cli::reason(&e)), ToastKind::Error, cx);
    }
    cx.notify();
}

pub(crate) fn read_offers(store: &mut Store, cx: &mut Context<Store>) {
    let asked = store.plugins.ask();
    store::run_json(cx, args(&["plugin", "actions", "--json"]), move |this, answer, cx| {
        let read = this.plugins.read_actions(asked, answer);
        offers_read(this, read, cx);
    });
    store::run_json(cx, args(&["plugin", "status", "--json"]), move |this, answer, cx| {
        let read = this.plugins.read_status(asked, answer);
        offers_read(this, read, cx);
    });
}

struct Told {
    said: Option<(String, ToastKind)>,
    refused: bool,
}

fn refused(text: String) -> Told {
    Told { said: Some((text, ToastKind::Error)), refused: true }
}

fn told(title: &str, popup: bool, answer: Result<String>) -> Told {
    let out = match answer {
        Ok(out) => out,
        Err(e) if popup => return refused(format!("{title}: {e} ({POPUP_WAITS})")),
        Err(e) => return refused(format!("{title}: {e}")),
    };
    let said = match serde_json::from_str::<Ran>(&out) {
        Ok(ran) => ran.say(title),
        Err(e) => Some((format!("{title}: {e}"), ToastKind::Error)),
    };
    Told { said, refused: false }
}

pub(crate) fn run_action<T: 'static>(
    run: &Run,
    cx: &mut Context<T>,
    then: impl FnOnce(&mut T, &mut Context<T>) + 'static,
) {
    if run.action.popup() {
        Store::get(cx).update(cx, Store::close_sheet);
    }
    let (title, popup) = (run.title().to_string(), run.action.popup());
    let command = run.command();
    let doing = job(&run.action.key(), format!("{title}…"));
    let work = move || cli::run_printed(&command);
    store::run_local_busy(cx, doing, work, move |this, answer, cx| {
        let told = told(&title, popup, answer);
        Store::get(cx).update(cx, |s, cx| {
            if let Some((text, kind)) = told.said {
                s.toast(text, kind, cx);
            }
            if told.refused {
                read_offers(s, cx);
            }
        });
        then(this, cx);
    });
}

pub(super) fn target(contexts: &[String], shown: Shown) -> Option<Spot> {
    if contexts.iter().any(|context| context == Spot::Button.context()) {
        return Some(Spot::Button);
    }
    let spots = [
        shown.pane.map(Spot::Pane),
        shown.tab.map(Spot::Tab),
        shown.workspace.map(Spot::Workspace),
        shown.project.map(Spot::Project),
    ];
    spots.into_iter().flatten().find(|spot| contexts.iter().any(|context| context == spot.context()))
}

#[cfg(test)]
pub(crate) fn listed_offers() -> Offers {
    use crate::test_util::fixture;
    let mut offers = Offers::default();
    let asked = offers.ask();
    let actions = serde_json::from_str(fixture!("plugins/actions.json")).map_err(anyhow::Error::from);
    let status = serde_json::from_str(fixture!("plugins/status-buttons.json")).map_err(anyhow::Error::from);
    offers.read_actions(asked, actions).expect("the actions parse");
    offers.read_status(asked, status).expect("the status parses");
    offers
}

#[cfg(test)]
mod tests;
