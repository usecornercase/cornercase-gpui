use std::collections::HashMap;
use std::rc::Rc;

use gpui::{Context, ListOffset, px};

use crate::cli::{self, Target};
use crate::model::ReadReleases;
use crate::store::{self, Busy, Store, ToastKind, args, read_refusal};

use super::pieces::{find, looks, parse, toast};
use super::wire::{Base, Gap, Patch};
use super::{ChangesPanel, DiffData, Gaps, Open, Scope, Selected, Whose, diff};

fn file_command(verb: &str, selected: &Selected, extra: &[String], releases: ReadReleases) -> Vec<String> {
    let mut a = args(&["changes", verb, &selected.path]);
    a.extend(extra.iter().cloned());
    a.extend(args(&["--workspace", &selected.workspace.arg(), "--mode", selected.mode.name(), "--json"]));
    selected.workspace.reading(a, releases)
}

fn refusal_text(machine: Option<&str>, error: &anyhow::Error) -> String {
    machine.and_then(|machine| read_refusal(machine, error)).unwrap_or_else(|| error.to_string())
}

pub(super) fn switching_base(workspace: u64) -> String {
    format!("changes-base:{workspace}")
}

impl Open {
    fn read_failed(&mut self, why: String) -> bool {
        self.digest.clear();
        self.error = Some(why);
        self.data.is_some()
    }

    pub(super) fn drop_reads_under_way(&mut self) {
        self.generation += 1;
        self.loading = false;
        self.digest.clear();
    }
}

impl ChangesPanel {
    fn read_target(&self, workspace: &Whose) -> Result<Target, String> {
        if matches!(workspace, Whose::Here(_)) {
            return Ok(cli::target());
        }
        let Some(read) = &self.on_machine else { return Err("that machine is no longer shown".to_string()) };
        self.hearing.may_read(read)
    }

    fn patch_failed(&mut self, machine: Option<&str>, error: &anyhow::Error, cx: &mut Context<Self>) {
        self.hearing.read_refused(error);
        let text = refusal_text(machine, error);
        if self.open.read_failed(text.clone()) {
            toast(text, ToastKind::Error, cx);
        }
    }

    pub(super) fn refuses_reading(&self, workspace: &Whose, cx: &mut Context<Self>) -> bool {
        let Err(why) = self.read_target(workspace) else { return false };
        Store::get(cx).update(cx, |s, cx| s.not_sent(&why, cx));
        true
    }

    pub(super) fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if scope == self.scope {
            return;
        }
        if let Scope::Project(mode) = scope {
            self.project_mode = mode;
        }
        self.scope = scope;
        self.base_open = false;
        self.select(None, cx);
        self.sync(cx);
        cx.notify();
    }

    pub(super) fn select(&mut self, selected: Option<Selected>, cx: &mut Context<Self>) {
        if selected == self.selected {
            return;
        }
        self.selected = selected;
        self.open = Open { generation: self.open.generation + 1, ..Open::default() };
        self.diff_list.reset(0);
        self.reread(cx);
        cx.notify();
    }

    pub(super) fn reread(&mut self, cx: &mut Context<Self>) {
        let Some(selected) = &self.selected else { return };
        let digest = self
            .summary
            .as_ref()
            .and_then(|s| find(s, &selected.workspace, &selected.path))
            .map(|f| f.digest.clone())
            .unwrap_or_default();
        self.load_patch(digest, cx);
    }

    pub(super) fn load_patch(&mut self, digest: String, cx: &mut Context<Self>) {
        let Some(selected) = self.selected.clone() else { return };
        let target = match self.read_target(&selected.workspace) {
            Ok(target) => target,
            Err(why) => {
                self.open.error = Some(why);
                cx.notify();
                return;
            }
        };
        self.open.digest = digest;
        self.open.loading = true;
        self.open.generation += 1;
        let generation = self.open.generation;
        let releases = selected.workspace.releases(Store::get(cx).read(cx));
        let a = file_command("file", &selected, &[], releases);
        let machine = self.machine_followed();
        store::run_on(cx, target, a, move |this: &mut Self, result, cx| {
            if this.open.generation != generation {
                return;
            }
            this.open.loading = false;
            match parse::<Patch>(result) {
                Ok(patch) => this.show_patch(selected, patch),
                Err(e) => this.patch_failed(machine.as_deref(), &e, cx),
            }
            cx.notify();
        });
        cx.notify();
    }

    fn show_patch(&mut self, selected: Selected, patch: Patch) {
        let same = self.open.data.as_ref().is_some_and(|d| d.selected == selected);
        let gaps = match &self.open.data {
            Some(d) if same && d.patch.hunks == patch.hunks => d.gaps.clone(),
            _ => {
                self.open.selecting.selection = None;
                HashMap::new()
            }
        };
        self.open.error = None;
        self.set_data(selected, patch, gaps, same);
    }

    fn set_data(&mut self, selected: Selected, patch: Patch, gaps: Gaps, keep_scroll: bool) {
        let looks = looks(&patch.styles);
        let gap_lines: HashMap<usize, Gap> = gaps.iter().map(|(k, (g, _))| (*k, g.clone())).collect();
        let rows = diff::rows(&patch.hunks, &gap_lines);
        let top = self.diff_list.logical_scroll_top();
        self.diff_list.reset(rows.len());
        if keep_scroll && top.item_ix < rows.len() {
            self.diff_list.scroll_to(top);
        } else {
            self.diff_list.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) });
        }
        self.open.data = Some(Rc::new(DiffData { selected, patch, looks, gaps, rows }));
    }

    pub(super) fn load_gap(&mut self, hunk: usize, cx: &mut Context<Self>) {
        let Some(data) = self.open.data.clone() else { return };
        if self.refuses_reading(&data.selected.workspace, cx) {
            return;
        }
        let Ok(target) = self.read_target(&data.selected.workspace) else { return };
        let releases = data.selected.workspace.releases(Store::get(cx).read(cx));
        let a = file_command("gap", &data.selected, &args(&["--hunk", &hunk.to_string()]), releases);
        let machine = self.machine_followed();
        let generation = self.open.generation;
        store::run_on(cx, target, a, move |this: &mut Self, result, cx| {
            if this.open.generation != generation {
                return;
            }
            match parse::<Gap>(result) {
                Ok(gap) => this.add_gap(hunk, gap),
                Err(e) => {
                    this.hearing.read_refused(&e);
                    toast(refusal_text(machine.as_deref(), &e), ToastKind::Error, cx);
                }
            }
            cx.notify();
        });
    }

    fn add_gap(&mut self, hunk: usize, gap: Gap) {
        let Some(data) = self.open.data.clone() else { return };
        let mut gaps = data.gaps.clone();
        let gap_looks = looks(&gap.styles);
        gaps.insert(hunk, (gap, gap_looks));
        self.set_data(data.selected.clone(), data.patch.clone(), gaps, true);
    }

    pub(super) fn followed_here(&self) -> Option<u64> {
        self.follow.as_ref()?.workspace.as_ref()?.here()
    }

    pub(super) fn open_base(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.followed_here() else { return };
        self.base_open = !self.base_open;
        if self.base_open {
            let a = args(&["changes", "base", "--workspace", &workspace.to_string(), "--json"]);
            store::run(cx, a, |this: &mut Self, result, cx| {
                match parse::<Base>(result) {
                    Ok(base) => this.base = Some(base),
                    Err(e) => this.base_failed(&e.to_string(), cx),
                }
                cx.notify();
            });
        }
        cx.notify();
    }

    fn base_failed(&mut self, error: &str, cx: &mut Context<Self>) {
        self.base_open = false;
        toast(error, ToastKind::Error, cx);
        cx.notify();
    }

    pub(super) fn choose_base(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        self.base_open = false;
        let Some(workspace) = self.followed_here() else { return };
        let mut a = args(&["changes", "base", "--workspace", &workspace.to_string()]);
        let label = branch
            .as_ref()
            .map_or_else(|| "Going back to the default base…".into(), |b| format!("Comparing with {b}…"));
        a.push(branch.unwrap_or_else(|| "--default".into()));
        if let Some(base) = &mut self.base {
            base.chosen = None;
        }
        let busy = Busy::new(switching_base(workspace), label);
        Store::get(cx).update(cx, |s, cx| s.run_aside(busy, a, cx));
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
