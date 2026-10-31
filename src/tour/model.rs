use serde::Deserialize;
use serde_json::Value;

use crate::panels::PanelKind;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Catalogue {
    pub(crate) state: State,
    pub(crate) tours: Vec<Tour>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct State {
    pub(crate) active: Option<Active>,
    pub(crate) progress: Vec<Progress>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct Active {
    pub(crate) tour: String,
    pub(crate) step: String,
    pub(crate) index: usize,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct Progress {
    pub(crate) tour: String,
    pub(crate) done: Vec<String>,
    pub(crate) finished: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Tour {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) docs: Option<String>,
    pub(crate) steps: Vec<Step>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Step {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) text: String,
    pub(crate) target: String,
    pub(crate) done: Value,
    pub(crate) show: Option<Value>,
    pub(crate) offer: Option<String>,
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn panel_named(name: &str) -> Option<PanelKind> {
    Some(match name {
        "changes" => PanelKind::Changes,
        "files" => PanelKind::Files,
        "todo" => PanelKind::Todo,
        "flow" | "flows" => PanelKind::Flows,
        "issues" => PanelKind::Issues,
        "queue" => PanelKind::Queue,
        "search" => PanelKind::Search,
        "settings" => PanelKind::Settings,
        _ => return None,
    })
}

impl Step {
    pub(crate) fn explains(&self) -> bool {
        field(&self.done, "kind") == Some("next")
    }

    pub(crate) fn shows(&self) -> Option<PanelKind> {
        let show = self.show.as_ref()?;
        match field(show, "kind")? {
            "panel" => panel_named(field(show, "panel")?),
            kind => panel_named(kind),
        }
    }

    pub(crate) fn completed_by(&self) -> Option<PanelKind> {
        match field(&self.done, "kind")? {
            "panel" => panel_named(field(&self.done, "panel")?),
            "opened" => panel_named(field(&self.done, "dialog")?),
            _ => None,
        }
    }

    pub(crate) fn points_at(&self) -> Option<PanelKind> {
        panel_named(&self.target)
    }

    pub(crate) fn offers_a_prompt(&self) -> bool {
        self.offer.as_deref() == Some("prompt")
    }
}

impl Catalogue {
    pub(crate) fn tour(&self, id: &str) -> Option<&Tour> {
        self.tours.iter().find(|t| t.id == id)
    }

    pub(crate) fn current(&self) -> Option<(&Tour, usize, &Step)> {
        let active = self.state.active.as_ref()?;
        let tour = self.tour(&active.tour)?;
        let index = tour.steps.iter().position(|s| s.id == active.step)?;
        Some((tour, index, &tour.steps[index]))
    }

    pub(crate) fn progress_label(&self, tour: &Tour) -> String {
        let progress = self.state.progress.iter().find(|p| p.tour == tour.id);
        if progress.is_some_and(|p| p.finished) {
            return "done".into();
        }
        let done = progress.map_or(0, |p| p.done.len());
        format!("{done}/{}", tour.steps.len())
    }
}

pub(crate) fn state_of(app: Option<&Value>) -> State {
    app.and_then(|a| a.get("tour")).and_then(|t| serde_json::from_value(t.clone()).ok()).unwrap_or_default()
}
