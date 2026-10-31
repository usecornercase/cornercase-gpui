use std::collections::{HashMap, HashSet};
use std::iter::successors;

use serde::Deserialize;

use crate::model::Session;
use crate::panels::files::disk::human_size;

const COMMAND_CHARS: usize = 60;
const ARGUMENT_CHARS: usize = 240;
const BUSY: f64 = 5.;
const HOT: f64 = 50.;
const SHOWN_AS_ZERO: f64 = 0.05;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Sample {
    pub(super) interval: u64,
    pub(super) total: Total,
    pub(super) panes: Vec<PaneUse>,
    pub(super) cornercase: Option<Own>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Total {
    pub(super) cpu: f64,
    pub(super) memory: u64,
    pub(super) panes: usize,
    pub(super) processes: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct PaneUse {
    pub(super) pane: u64,
    pub(super) tab: u64,
    pub(super) workspace: u64,
    pub(super) project: u64,
    pub(super) program: Option<String>,
    pub(super) agent: Option<String>,
    pub(super) pid: u64,
    pub(super) cpu: Option<f64>,
    pub(super) memory: Option<u64>,
    pub(super) processes: Vec<Process>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Own {
    pub(super) pid: u64,
    pub(super) cpu: Option<f64>,
    pub(super) memory: Option<u64>,
    pub(super) processes: Vec<Process>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Process {
    pub(super) pid: u64,
    pub(super) parent: Option<u64>,
    pub(super) name: String,
    pub(super) args: Vec<String>,
    pub(super) started: Option<u64>,
    pub(super) cpu: Option<f64>,
    pub(super) memory: Option<u64>,
}

impl Process {
    pub(super) fn command(&self) -> (String, String) {
        let (program, arguments) = command(&self.name, &self.args);
        (program, cut(&arguments.replace(['\n', '\r', '\t'], " "), ARGUMENT_CHARS))
    }

    pub(super) fn stop_command(&self) -> Vec<String> {
        let mut command = vec!["processes".to_string(), "stop".to_string(), self.pid.to_string()];
        if let Some(started) = self.started {
            command.extend(["--started".to_string(), started.to_string()]);
        }
        command
    }

    fn busy(&self) -> bool {
        self.cpu.is_some_and(|cpu| cpu >= BUSY)
    }

    fn mentions(&self, lowered: &str) -> bool {
        contains(&self.name, lowered) || contains(&self.args.join(" "), lowered)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Order {
    #[default]
    Sidebar,
    Cpu,
    Memory,
}

pub(super) fn ordered<'a>(panes: &'a [PaneUse], order: Order, held: Option<&[u64]>) -> Vec<&'a PaneUse> {
    let mut list: Vec<&PaneUse> = panes.iter().collect();
    match order {
        Order::Sidebar => {}
        Order::Cpu => list.sort_by(|a, b| b.cpu.unwrap_or(0.).total_cmp(&a.cpu.unwrap_or(0.))),
        Order::Memory => list.sort_by_key(|p| std::cmp::Reverse(p.memory.unwrap_or(0))),
    }
    if let Some(held) = held {
        list.sort_by_key(|p| held.iter().position(|id| *id == p.pane).unwrap_or(usize::MAX));
    }
    list
}

pub(super) fn tree(processes: &[Process]) -> Vec<(usize, &Process)> {
    let mut depths: HashMap<u64, usize> = HashMap::new();
    processes
        .iter()
        .map(|p| {
            let depth = p.parent.and_then(|parent| depths.get(&parent)).map_or(0, |d| d + 1);
            depths.insert(p.pid, depth);
            (depth, p)
        })
        .collect()
}

pub(super) fn under(processes: &[Process], root: u64) -> Vec<(usize, &Process)> {
    tree(processes).into_iter().filter(|(_, p)| p.pid != root).map(|(depth, p)| (depth.saturating_sub(1), p)).collect()
}

pub(super) struct Fold<'a> {
    pub(super) rows: Vec<(usize, &'a Process)>,
    pub(super) hidden: usize,
}

pub(super) fn folded<'a>(rows: Vec<(usize, &'a Process)>, open: bool, found_by: &str) -> Fold<'a> {
    let kept = kept(&rows, &found_by.trim().to_lowercase());
    let hidden = rows.len().saturating_sub(kept.len());
    let rows = if open { rows } else { rows.into_iter().filter(|(_, p)| kept.contains(&p.pid)).collect() };
    Fold { rows, hidden }
}

fn kept(rows: &[(usize, &Process)], lowered: &str) -> HashSet<u64> {
    let picked = rows.iter().filter(|(depth, p)| match lowered {
        "" => *depth == 0 || p.busy(),
        _ => p.mentions(lowered),
    });
    let parents: HashMap<u64, Option<u64>> = rows.iter().map(|(_, p)| (p.pid, p.parent)).collect();
    let line = |pid: u64| -> Vec<u64> {
        successors(Some(pid), |at| parents.get(at).copied().flatten())
            .take(rows.len())
            .filter(|at| parents.contains_key(at))
            .collect()
    };
    picked.flat_map(|(_, p)| line(p.pid)).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Found {
    Not,
    Pane,
    Inside,
}

pub(super) fn found(pane: &PaneUse, place: &str, filter: &str) -> Found {
    let filter = filter.trim().to_lowercase();
    if filter.is_empty() || contains(place, &filter) || contains(&heading(pane), &filter) {
        Found::Pane
    } else if pane.processes.iter().any(|p| p.mentions(&filter)) {
        Found::Inside
    } else {
        Found::Not
    }
}

fn contains(text: &str, lowered: &str) -> bool {
    text.to_lowercase().contains(lowered)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Heat {
    Idle,
    Busy,
    Hot,
}

pub(super) fn heat(cpu: Option<f64>) -> Heat {
    match cpu {
        Some(value) if value >= HOT => Heat::Hot,
        Some(value) if value >= SHOWN_AS_ZERO => Heat::Busy,
        _ => Heat::Idle,
    }
}

#[expect(clippy::cast_possible_truncation, reason = "a share drawn on screen needs no f64 precision")]
pub(super) fn load(cpu: Option<f64>) -> f32 {
    (cpu.unwrap_or(0.) / 100.).clamp(0., 1.) as f32
}

pub(super) fn sample_command(workspace: Option<u64>) -> Vec<String> {
    let mut command = vec!["processes".to_string(), "--json".to_string()];
    if let Some(id) = workspace {
        command.extend(["--workspace".to_string(), id.to_string()]);
    }
    command
}

pub(super) fn cpu(value: Option<f64>) -> String {
    value.map_or_else(|| "?".into(), |v| format!("{v:.1} %"))
}

pub(super) fn memory(value: Option<u64>) -> String {
    value.map_or_else(|| "?".into(), human_size)
}

pub(super) fn counts(sample: &Sample) -> String {
    let t = &sample.total;
    format!("{} in {}", count(t.processes, "process", "processes"), count(t.panes, "pane", "panes"))
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub(super) fn place(pane: &PaneUse, session: Option<&Session>) -> String {
    let tab = session.and_then(|s| s.tab(pane.tab));
    let workspace = session.and_then(|s| s.workspace(pane.workspace));
    let mut parts: Vec<String> = Vec::new();
    if let Some((project, ws)) = workspace {
        parts.extend([project.name.clone(), ws.name.clone()]);
    }
    parts.extend(tab.map(|t| t.name.clone()));
    let line = parts.join(" › ");
    let split = tab.is_none_or(|t| t.panes.len() > 1);
    match (line.is_empty(), split) {
        (true, _) => format!("pane {}", pane.pane),
        (false, true) => format!("{line} · pane {}", pane.pane),
        (false, false) => line,
    }
}

pub(super) fn heading(pane: &PaneUse) -> String {
    pane.agent.clone().or_else(|| pane.program.clone()).unwrap_or_else(|| "shell".into())
}

pub(crate) fn command_line(name: &str, args: &[String]) -> String {
    let (program, arguments) = command(name, args);
    let line = if arguments.is_empty() { program } else { format!("{program} {arguments}") };
    cut(&line, COMMAND_CHARS)
}

fn command(name: &str, args: &[String]) -> (String, String) {
    let program = args.first().and_then(|a| a.rsplit('/').next()).filter(|a| !a.is_empty()).unwrap_or(name);
    (program.to_string(), args.iter().skip(1).map(String::as_str).collect::<Vec<_>>().join(" "))
}

fn cut(text: &str, chars: usize) -> String {
    if text.chars().count() <= chars {
        return text.to_string();
    }
    let kept: String = text.chars().take(chars.saturating_sub(1)).collect();
    format!("{kept}…")
}
