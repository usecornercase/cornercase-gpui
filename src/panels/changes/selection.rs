use std::collections::HashMap;
use std::rc::Rc;

use gpui::{Context, ListOffset, px};

use crate::store::{self, Store, ToastKind, args};

use super::pieces::{find, looks, parse, toast};
use super::wire::{Base, Gap, Patch};
use super::{ChangesPanel, DiffData, Gaps, Open, Scope, Selected, diff};

fn file_command(verb: &str, selected: &Selected, extra: &[String]) -> Vec<String> {
    let mut a = args(&["changes", verb, &selected.path]);
    a.extend(extra.iter().cloned());
    a.extend(args(&["--workspace", &selected.workspace.to_string(), "--mode", selected.mode.name(), "--json"]));
    a
}

impl ChangesPanel {
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
    }

    pub(super) fn select(&mut self, selected: Option<Selected>, cx: &mut Context<Self>) {
        if selected == self.selected {
            return;
        }
        self.selected = selected;
        self.open = Open { generation: self.open.generation + 1, ..Open::default() };
        self.diff_list.reset(0);
        if let Some(selected) = &self.selected {
            let digest = self
                .summary
                .as_ref()
                .and_then(|s| find(s, selected.workspace, &selected.path))
                .map(|f| f.digest.clone())
                .unwrap_or_default();
            self.load_patch(digest, cx);
        }
        cx.notify();
    }

    pub(super) fn load_patch(&mut self, digest: String, cx: &mut Context<Self>) {
        let Some(selected) = self.selected.clone() else { return };
        self.open.digest = digest;
        self.open.loading = true;
        self.open.generation += 1;
        let generation = self.open.generation;
        let a = file_command("file", &selected, &[]);
        store::run(cx, a, move |this: &mut Self, result, cx| {
            if this.open.generation != generation {
                return;
            }
            this.open.loading = false;
            match parse::<Patch>(result) {
                Ok(patch) => this.show_patch(selected, patch),
                Err(e) => this.open.error = Some(e.to_string()),
            }
            cx.notify();
        });
        cx.notify();
    }

    fn show_patch(&mut self, selected: Selected, patch: Patch) {
        let same = self.open.data.as_ref().is_some_and(|d| d.selected == selected);
        let gaps = match &self.open.data {
            Some(d) if same && d.patch.hunks == patch.hunks => d.gaps.clone(),
            _ => HashMap::new(),
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
        let a = file_command("gap", &data.selected, &args(&["--hunk", &hunk.to_string()]));
        let generation = self.open.generation;
        store::run(cx, a, move |this: &mut Self, result, cx| {
            if this.open.generation != generation {
                return;
            }
            match parse::<Gap>(result) {
                Ok(gap) => this.add_gap(hunk, gap),
                Err(e) => toast(e.to_string(), ToastKind::Error, cx),
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

    pub(super) fn open_base(&mut self, cx: &mut Context<Self>) {
        self.base_open = !self.base_open;
        let Some(workspace) = self.follow.and_then(|f| f.workspace) else { return };
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
    }

    pub(super) fn choose_base(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        self.base_open = false;
        let Some(workspace) = self.follow.and_then(|f| f.workspace) else { return };
        let mut a = args(&["changes", "base", "--workspace", &workspace.to_string()]);
        a.push(branch.unwrap_or_else(|| "--default".into()));
        if let Some(base) = &mut self.base {
            base.chosen = None;
        }
        Store::get(cx).read(cx).cmd(a);
        cx.notify();
    }
}
