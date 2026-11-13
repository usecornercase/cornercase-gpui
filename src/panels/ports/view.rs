use std::collections::HashSet;

use super::data::Port;
use crate::model::{MachineSeen, Session, ThereId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Go {
    Here(u64),
    There { machine: String, boot: String, pane: ThereId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PortView {
    pub(crate) key: String,
    pub(crate) number: u16,
    pub(crate) url: Option<String>,
    pub(crate) addresses: Vec<String>,
    pub(crate) pid: u64,
    pub(crate) program: String,
    pub(crate) command_line: String,
    pub(crate) place: String,
    pub(crate) by_agent: bool,
    pub(crate) by_folder: bool,
    pub(crate) stoppable: bool,
    pub(crate) go: Option<Go>,
}

impl PortView {
    pub(crate) fn stop_command(&self) -> Vec<String> {
        ["ports", "stop", &self.number.to_string(), "--pid", &self.pid.to_string()].map(str::to_string).to_vec()
    }
}

fn plain<Id>(port: &Port<Id>, key: String, place: String, go: Option<Go>) -> PortView {
    PortView {
        key,
        number: port.number,
        url: port.url.clone(),
        addresses: port.addresses.clone(),
        pid: port.pid,
        program: port.program.clone(),
        command_line: port.command_line(),
        place,
        by_agent: port.by_agent(),
        by_folder: port.pane.is_none(),
        stoppable: port.stoppable(),
        go,
    }
}

pub(crate) fn there_key<Id>(port: &Port<Id>) -> String {
    format!("there-{}-{}", port.number, port.pid)
}

pub(crate) fn view_here(port: &Port, session: Option<&Session>, with_workspace: bool) -> PortView {
    let key = format!("{}-{}", port.number, port.pid);
    plain(port, key, port.place(session, with_workspace), port.pane.map(Go::Here))
}

pub(crate) fn view_there(port: &Port<ThereId>, seen: Option<&MachineSeen>, with_workspace: bool) -> PortView {
    let pane = port.pane.map(|p| (p.label(), seen.and_then(|s| s.tab_of_pane(p)).map(|tab| tab.name.clone())));
    let how = port.how(pane);
    let workspace = port.workspace.filter(|_| with_workspace).and_then(|id| seen?.place_name(id));
    let place = workspace.map_or_else(|| how.clone(), |name| format!("{name} · {how}"));
    let key = there_key(port);
    let go = seen.zip(port.pane).map(|(s, pane)| Go::There { machine: s.name.clone(), boot: s.boot.clone(), pane });
    plain(port, key, place, go)
}

pub(crate) fn stop_warning(view: &PortView, on: Option<&str>) -> String {
    let there = on.map(|machine| format!(" on {machine}")).unwrap_or_default();
    let mut text = format!(
        "{} (pid {}){there} gets SIGTERM, then SIGKILL if port {} is still held after 3 s. \
         Only that process is signalled; the shell or agent that started it stays.",
        view.program, view.pid, view.number
    );
    if view.by_agent {
        text.push_str(" An agent started it, or it is the agent itself.");
    }
    if view.by_folder {
        text.push_str(" No tab started it: it was found by its folder.");
    }
    text
}

pub(crate) fn to_stop<'a>(views: &'a [PortView], ticked: &HashSet<String>, every_port: bool) -> Vec<&'a PortView> {
    let stoppable = views.iter().filter(|p| p.stoppable);
    if ticked.is_empty() && !every_port {
        return stoppable.collect();
    }
    stoppable.filter(|p| ticked.contains(&p.key)).collect()
}

pub(crate) fn stop_label(count: usize, ticked: bool) -> String {
    if ticked { format!("Stop {count}…") } else { "Stop all…".to_string() }
}

pub(crate) fn stop_many_warning(views: &[&PortView], on: Option<&str>) -> String {
    let there = on.map(|machine| format!(" on {machine}")).unwrap_or_default();
    let named: Vec<String> =
        views.iter().map(|p| format!("{} on port {} (pid {}){there}", p.program, p.number, p.pid)).collect();
    let mut text = format!(
        "{} each get SIGTERM, then SIGKILL if the port is still held after 3 s. Only those processes are signalled; \
         the shells and agents that started them stay.",
        named.join(", ")
    );
    if views.iter().any(|p| p.by_agent) {
        text.push_str(" An agent started some of them.");
    }
    if views.iter().any(|p| p.by_folder) {
        text.push_str(" No tab started some of them: they were found by their folder.");
    }
    text
}

pub(crate) fn stop_title(view: &PortView, on: Option<&str>) -> String {
    match on {
        Some(machine) => format!("Stop {} on port {} on {machine}?", view.program, view.number),
        None => format!("Stop {} on port {}?", view.program, view.number),
    }
}

pub(crate) fn stop_many_title(count: usize, on: Option<&str>) -> String {
    match on {
        Some(machine) => format!("Stop {count} programs on {machine}?"),
        None => format!("Stop {count} programs?"),
    }
}
