use serde::Deserialize;
use serde_json::Value;

use crate::model::Shown;
use crate::store::ToastKind;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct List {
    pub(super) plugins: Vec<Plugin>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Plugin {
    pub(super) id: String,
    pub(super) enabled: bool,
    pub(super) source: Source,
    pub(super) manifest: Option<Manifest>,
    pub(super) error: Option<String>,
}

impl Plugin {
    pub(super) fn name(&self) -> String {
        self.manifest.as_ref().map_or_else(|| self.id.clone(), |m| m.name.clone())
    }

    pub(super) fn facts(&self) -> String {
        let mut facts = vec![self.source.describe()];
        if let Some(m) = &self.manifest {
            facts.push(m.asks());
            let runs = m.runs();
            if !runs.is_empty() {
                facts.push(format!("runs {}", runs.join(", ")));
            }
        }
        facts.join(" · ")
    }

    pub(super) fn enable_warning(&self) -> String {
        let asks = self.manifest.as_ref().map(Manifest::asks).unwrap_or_default();
        let runs = self.manifest.as_ref().map(Manifest::runs).unwrap_or_default();
        let runs = if runs.is_empty() { String::new() } else { format!(" It runs {}.", runs.join(", ")) };
        format!(
            "{} runs as you and can do anything you can, including typing into every terminal. \
             It {asks}; that is shown, not enforced.{runs}",
            self.name()
        )
    }

    pub(super) fn has_service(&self) -> bool {
        self.manifest.as_ref().is_some_and(|m| !m.service.is_empty())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub(super) struct Source(Value);

impl Source {
    fn field(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
    }

    pub(super) fn installed(&self) -> bool {
        self.field("kind") == Some("git")
    }

    pub(super) fn describe(&self) -> String {
        match self.field("kind") {
            Some("link") => format!("linked from {}", self.field("path").unwrap_or("?")),
            Some("git") => self.describe_git(),
            Some(kind) => kind.to_string(),
            None => String::new(),
        }
    }

    fn describe_git(&self) -> String {
        let place = self.field("repo").or(self.field("url")).unwrap_or("?");
        let subdir = self.field("subdir").map(|s| format!("/{s}")).unwrap_or_default();
        let at = match (self.field("ref"), self.field("commit")) {
            (Some(r), Some(c)) => format!(" @{r} ({})", short(c)),
            (None, Some(c)) => format!(" @{}", short(c)),
            (Some(r), None) => format!(" @{r}"),
            (None, None) => String::new(),
        };
        format!("installed from {place}{subdir}{at}")
    }
}

fn short(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Manifest {
    pub(super) name: String,
    pub(super) version: String,
    pub(super) description: Option<String>,
    pub(super) permissions: Vec<String>,
    pub(super) actions: Vec<Value>,
    pub(super) events: Vec<Value>,
    pub(super) service: Vec<Value>,
    pub(super) window: Vec<Value>,
}

fn count(n: usize, one: &str, many: &str) -> Option<String> {
    match n {
        0 => None,
        1 => Some(format!("1 {one}")),
        n => Some(format!("{n} {many}")),
    }
}

impl Manifest {
    pub(super) fn asks(&self) -> String {
        if self.permissions.is_empty() {
            "asks for nothing".into()
        } else {
            format!("asks for {}", self.permissions.join(", "))
        }
    }

    pub(super) fn runs(&self) -> Vec<String> {
        [
            count(self.actions.len(), "action", "actions"),
            count(self.events.len(), "hook", "hooks"),
            count(self.service.len(), "service", "services"),
            count(self.window.len(), "window process", "window processes"),
        ]
        .into_iter()
        .flatten()
        .collect()
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
    #[serde(rename = "action")]
    pub(super) name: String,
    pub(super) title: String,
    pub(super) contexts: Vec<String>,
    pub(super) placement: String,
}

impl Action {
    pub(super) fn key(&self) -> String {
        format!("{}/{}", self.plugin, self.name)
    }

    pub(super) fn popup(&self) -> bool {
        self.placement == "popup"
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Ran {
    pub(super) plugin: String,
    pub(super) error: Option<String>,
    pub(super) ms: u64,
}

impl Ran {
    pub(super) fn say(&self, title: &str) -> (String, ToastKind) {
        match &self.error {
            None => (format!("{title}: done in {}", duration(self.ms)), ToastKind::Info),
            Some(e) => (format!("{title}: {e}; `cornercase plugin logs {}` says more", self.plugin), ToastKind::Error),
        }
    }
}

pub(super) fn duration(ms: u64) -> String {
    if ms < 1000 {
        return format!("{ms} ms");
    }
    format!("{:.1} s", f64::from(u32::try_from(ms).unwrap_or(u32::MAX)) / 1000.)
}

pub(super) fn target(contexts: &[String], shown: Shown) -> Option<Vec<String>> {
    if contexts.iter().any(|c| c == "button") {
        return Some(Vec::new());
    }
    [("pane", shown.pane), ("tab", shown.tab), ("workspace", shown.workspace), ("project", shown.project)]
        .into_iter()
        .find_map(|(kind, id)| id.filter(|_| contexts.iter().any(|c| c == kind)).map(|id| (kind, id)))
        .map(|(kind, id)| vec![format!("--{kind}"), id.to_string()])
}

pub(super) fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().trim().to_string()
}

pub(super) fn shell_quote(word: &str) -> String {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:@=+,".contains(c)) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}
