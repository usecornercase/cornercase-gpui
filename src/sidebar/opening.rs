use std::path::{Path, PathBuf};

use futures::channel::oneshot;
use gpui::{App, Context, PathPromptOptions, Window};
use serde::Deserialize;

use super::{Fold, cmd};
use crate::app::Root;
use crate::cli;
use crate::dialogs::{self, PathQuestion};
use crate::platform::paths::Style;
use crate::store::{self, Busy, ToastKind};

const CODE_WORKSPACE: &str = ".code-workspace";

type Picked = oneshot::Receiver<anyhow::Result<Option<Vec<PathBuf>>>>;
type Act = fn(&mut Root, &str, Option<u64>, &mut Context<Root>);

#[derive(Debug, Deserialize)]
struct Imported {
    group: Option<u64>,
    name: String,
    projects: Vec<u64>,
    missing: usize,
    failed: usize,
}

impl Imported {
    fn message(&self) -> String {
        let imported = match self.projects.len() {
            0 => format!("nothing imported from {}", self.name),
            1 => format!("1 project imported into {}", self.name),
            n => format!("{n} projects imported into {}", self.name),
        };
        let missing = (self.missing > 0).then(|| format!("{} not found", self.missing));
        let failed = (self.failed > 0).then(|| format!("{} could not open, see server.log", self.failed));
        [Some(imported), missing, failed].into_iter().flatten().collect::<Vec<_>>().join(", ")
    }

    fn kind(&self) -> ToastKind {
        if self.failed > 0 || self.projects.is_empty() { ToastKind::Error } else { ToastKind::Done }
    }

    fn group_to_unfold(&self) -> Option<u64> {
        self.group.filter(|_| !self.projects.is_empty())
    }
}

fn file_name(path: &str) -> &str {
    Some(Style::of_path(path).name(path)).filter(|name| !name.is_empty()).unwrap_or(path)
}

fn opening(path: &str) -> Busy {
    Busy::new(format!("open:{path}"), format!("Opening {}…", file_name(path)))
}

fn imported_name(path: &str) -> &str {
    let name = file_name(path);
    name.strip_suffix(CODE_WORKSPACE).filter(|stem| !stem.is_empty()).unwrap_or(name)
}

fn importing(path: &str) -> Busy {
    Busy::new(format!("open:{path}"), format!("Importing {}…", imported_name(path)))
}

fn is_code_workspace(path: &str) -> bool {
    file_name(path).ends_with(CODE_WORKSPACE)
}

fn not_a_code_workspace(path: &str) -> String {
    format!("{} is not a VS Code workspace: choose a {CODE_WORKSPACE} file", file_name(path))
}

fn open_args(path: &str, group: Option<u64>) -> Vec<String> {
    let mut args = cmd(&["open", path]);
    if let Some(group) = group {
        args.extend(cmd(&["--into", &group.to_string()]));
    }
    args.push("--focus".into());
    args
}

fn import_args(path: &str, group: Option<u64>) -> Vec<String> {
    let mut args = open_args(path, group);
    args.push("--json".into());
    args
}

fn path_as_text(path: &Path) -> Result<&str, String> {
    path.to_str().ok_or_else(|| format!("{} is not valid UTF-8, so it cannot be passed to cornercase", path.display()))
}

fn take_picked(root: &mut Root, paths: &[PathBuf], group: Option<u64>, then: Act, cx: &mut Context<Root>) {
    let Some(path) = paths.first() else { return };
    match path_as_text(path) {
        Ok(path) => then(root, path, group, cx),
        Err(why) => root.say(&why, cx),
    }
}

fn on_picked(picked: Picked, group: Option<u64>, then: Act, cx: &mut Context<Root>) {
    cx.spawn(async move |this, cx| {
        let answer = picked.await;
        let _ = this.update(cx, |this, cx| match answer {
            Ok(Ok(Some(paths))) => take_picked(this, &paths, group, then, cx),
            Ok(Err(e)) => this.say(&format!("The file picker could not open: {e}"), cx),
            _ => {}
        });
    })
    .detach();
}

fn ask_there(question: PathQuestion, group: Option<u64>, then: Act, window: &mut Window, cx: &mut Context<Root>) {
    let root = cx.entity();
    let answered = move |path: String, _: &mut Window, cx: &mut App| {
        root.update(cx, |this, cx| then(this, &path, group, cx));
    };
    dialogs::ask_path(question, answered, window, cx);
}

impl Root {
    #[expect(clippy::unused_self, reason = "a menu item's action and ⌘O call it as a method of the window")]
    pub(crate) fn open_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        Self::open_project_into(None, window, cx);
    }

    pub(super) fn open_project_into(group: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        let verb = if group.is_some() { "Add" } else { "Open" };
        if let Some(host) = cli::host() {
            let question = PathQuestion::folder(format!("{verb} a project on {host}"), verb);
            ask_there(question, group, Self::open_typed, window, cx);
            return;
        }
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(format!("{verb} project").into()),
        });
        on_picked(rx, group, Self::open_folder, cx);
    }

    pub(super) fn import_code_workspace(group: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(host) = cli::host() {
            let question = PathQuestion::code_workspace(format!("Import a VS Code workspace on {host}"), "Import");
            ask_there(question, group, Self::import_file, window, cx);
            return;
        }
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        on_picked(rx, group, Self::import_file, cx);
    }

    fn open_typed(&mut self, path: &str, group: Option<u64>, cx: &mut Context<Self>) {
        if is_code_workspace(path) {
            self.import_file(path, group, cx);
        } else {
            self.open_folder(path, group, cx);
        }
    }

    fn open_folder(&mut self, path: &str, group: Option<u64>, cx: &mut Context<Self>) {
        let (busy, args) = (opening(path), open_args(path, group));
        let Some(group) = group else {
            self.store.update(cx, |s, cx| s.act_busy(busy, args, cx));
            return;
        };
        let args = self.store.read(cx).with_view(args);
        store::run_busy(cx, busy, args, move |this: &mut Root, result, cx| match result {
            Ok(_) => this.set_folded(Fold::Group(group), false),
            Err(e) => this.say(&cli::reason(&e), cx),
        });
    }

    fn import_file(&mut self, path: &str, group: Option<u64>, cx: &mut Context<Self>) {
        if !is_code_workspace(path) {
            self.say(&not_a_code_workspace(path), cx);
            return;
        }
        let busy = importing(path);
        let args = self.store.read(cx).with_view(import_args(path, group));
        store::run_partial_busy(cx, busy, args, |this: &mut Root, result, cx| {
            let answer = result.and_then(|out| serde_json::from_str::<Imported>(&out).map_err(Into::into));
            this.imported(answer, cx);
        });
    }

    fn imported(&mut self, answer: anyhow::Result<Imported>, cx: &mut Context<Self>) {
        match answer {
            Ok(imported) => {
                if let Some(group) = imported.group_to_unfold() {
                    self.set_folded(Fold::Group(group), false);
                }
                self.store.update(cx, |s, cx| s.toast(imported.message(), imported.kind(), cx));
            }
            Err(e) => self.say(&cli::reason(&e), cx),
        }
    }
}

#[cfg(test)]
mod tests;
