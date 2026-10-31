use gpui::{AppContext as _, Context, Window};

use super::fetch::{Checked, Preview, install_question, update_question};
use super::{PluginsPanel, busy, job, say};
use crate::cli;
use crate::store::{self, Busy, Store, args};
use crate::ui::TextField;

pub(super) const FETCHING: &str = "fetch";
pub(super) const INSTALLING: &str = "install";
pub(super) const LINKING: &str = "link";
pub(super) const UPDATING_ALL: &str = "update-all";

pub(super) fn checking(id: &str) -> String {
    format!("check:{id}")
}

fn preview<T: 'static>(
    (command, busy): (Vec<String>, Busy),
    window: &Window,
    cx: &mut Context<T>,
    then: impl FnOnce(&mut T, anyhow::Result<Preview>, &mut Window, &mut Context<T>) + 'static,
) {
    let store = Store::get(cx);
    let job = store.update(cx, |s, cx| s.start_job(busy, cx));
    let task = cx.background_spawn(async move { cli::run(&command).and_then(|out| Preview::parse(&out)) });
    cx.spawn_in(window, async move |this, cx| {
        let result = task.await;
        let _ = store.update(cx, |s, cx| s.end_job(job, cx));
        let _ = this.update_in(cx, |this, window, cx| then(this, result, window, cx));
    })
    .detach();
}

impl PluginsPanel {
    pub(super) fn install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let source = self.source.read(cx).text().trim().to_string();
        if source.is_empty() || busy(FETCHING, cx) || busy(INSTALLING, cx) {
            return;
        }
        let command = args(&["plugin", "install", &source, "--preview", "--json"]);
        let fetching = job(FETCHING, format!("Fetching {source}…"));
        preview((command, fetching), window, cx, move |_, result, window, cx| match result {
            Ok(preview) => Self::ask_install(source, preview, window, cx),
            Err(e) => say(Err(e), cx),
        });
    }

    fn ask_install(source: String, preview: Preview, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity();
        install_question(&preview).ask(window, cx, move |cx| {
            this.update(cx, |this, cx| this.confirm_install(&source, &preview, cx));
        });
    }

    fn confirm_install(&mut self, source: &str, preview: &Preview, cx: &mut Context<Self>) {
        if self.source.read(cx).text().trim() == source {
            self.source.update(cx, TextField::clear);
        }
        let command = preview.install_command(source);
        let installing = job(INSTALLING, format!("Installing {source}…"));
        Self::plugin_cmd_saying(command, installing, Some(preview.installed()), cx);
    }

    pub(super) fn check_updates(&mut self, ids: Vec<String>, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if ids.is_empty() || busy(key, cx) {
            return;
        }
        self.checks.insert(key.to_string(), Checked::new(ids.len()));
        for id in ids {
            let key = key.to_string();
            let command = args(&["plugin", "update", &id, "--preview", "--json"]);
            let checking = job(&key, format!("Checking {id} for updates…"));
            preview((command, checking), window, cx, move |this, result, window, cx| {
                this.checked(&key, result, window, cx);
            });
        }
    }

    fn checked(&mut self, key: &str, result: anyhow::Result<Preview>, window: &mut Window, cx: &mut Context<Self>) {
        if !self.checks.get_mut(key).is_some_and(|c| c.add(result)) {
            return;
        }
        if let Some(checked) = self.checks.remove(key) {
            Self::ask_update(checked, window, cx);
        }
    }

    fn ask_update(checked: Checked, window: &mut Window, cx: &mut Context<Self>) {
        for e in checked.failed.iter().cloned() {
            say(Err(anyhow::anyhow!(e)), cx);
        }
        if let Some(text) = checked.up_to_date() {
            say(Ok(text), cx);
        }
        if checked.changed.is_empty() {
            return;
        }
        let this = cx.entity();
        let question = update_question(&checked.changed);
        let mut changed = checked.changed;
        changed.reverse();
        question.ask(window, cx, move |cx| {
            this.update(cx, |this, cx| this.apply_updates(changed.clone(), cx));
        });
    }

    fn apply_updates(&mut self, mut left: Vec<Preview>, cx: &mut Context<Self>) {
        let Some(next) = left.pop() else {
            self.reload(cx);
            return;
        };
        let updating = job(&next.id, format!("Updating {}…", next.id));
        store::run_busy(cx, updating, next.update_command(), move |this, result, cx| {
            say(result.map(|_| next.updated()), cx);
            this.apply_updates(left, cx);
        });
    }
}
