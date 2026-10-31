use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Status {
    pub(super) plugins: Vec<PluginStatus>,
    pub(super) problem: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct PluginStatus {
    pub(super) id: String,
    pub(super) services: Vec<Service>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Service {
    pub(super) id: String,
    pub(super) state: String,
    pub(super) pid: Option<u32>,
    pub(super) restart_in: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Health {
    Running,
    Waiting,
    Stopped,
}

impl Status {
    pub(super) fn services(&self, id: &str) -> &[Service] {
        self.plugins.iter().find(|p| p.id == id).map_or(&[], |p| p.services.as_slice())
    }
}

impl Service {
    pub(super) fn describe(&self) -> String {
        match (self.state.as_str(), self.pid, self.restart_in) {
            ("running", Some(pid), _) => format!("{} running, pid {pid}", self.id),
            ("stopped", _, Some(0)) => format!("{} stopped, restarting now", self.id),
            ("stopped", _, Some(s)) => format!("{} stopped, restarting in {s} s", self.id),
            ("starting", _, _) => format!("{} starting", self.id),
            (state, _, _) => format!("{} {state}", self.id),
        }
    }

    pub(super) fn health(&self) -> Health {
        match self.state.as_str() {
            "running" => Health::Running,
            "stopped" => Health::Stopped,
            _ => Health::Waiting,
        }
    }
}

#[cfg(test)]
mod tests;
