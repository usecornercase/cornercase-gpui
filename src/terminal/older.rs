use gpui::{ClickEvent, Context, Div, ParentElement as _, Stateful, StatefulInteractiveElement as _, Window};

use super::no_tab::{answer_under, column, heading, lead_text, retry_button, row, scrolled, tall};
use crate::app::{Conn, Root};
use crate::cli::{self, Behind, Older, Target};
use crate::desktop;
use crate::dialogs;
use crate::keymap::{Command, with_keys};
use crate::platform::Os;
use crate::sidebar::tip;
use crate::store::Busy;
use crate::ui::{self, Icon, Tone};

const UPDATE: &str = "Update";
const RESTART: &str = "Restart";
const NEEDS: &str = "To connect, the window needs the same release there, or a newer one.";

fn older_heading(older: &Older) -> String {
    format!("{} runs an older cornercase", older.destination)
}

pub(crate) fn older_lead(older: &Older, os: Os) -> String {
    let (name, here) = (&older.destination, desktop::machine(os));
    let mine = &older.here;
    match &older.there {
        Some(there) => format!("{name} runs cornercase {there} and this {here} runs {mine}. {NEEDS}"),
        None => {
            format!("{name} runs a cornercase too old to say which release it is, and this {here} runs {mine}. {NEEDS}")
        }
    }
}

fn behind_heading(behind: &Behind) -> String {
    format!("The server on {} still runs {}", behind.destination, behind.server)
}

pub(crate) fn behind_lead(behind: &Behind, os: Os) -> String {
    format!(
        "cornercase is updated there, but its server keeps running the release it started with until it restarts, \
         and this {} runs {}. Restarting it stops every program running in its terminals.",
        desktop::machine(os),
        behind.here
    )
}

fn update_question(destination: &str, os: Os) -> (String, String, &'static str) {
    let title = format!("Update cornercase on {destination}?");
    let message = format!(
        "This runs `cornercase update` there from this {}, which installs the latest release and restarts nothing: \
         its server keeps running the one it has until it restarts there. The window then connects again.",
        desktop::machine(os)
    );
    (title, message, UPDATE)
}

fn restart_question(destination: &str) -> (String, String, &'static str) {
    let title = format!("Restart cornercase on {destination}?");
    let message = format!(
        "Restarting stops every program running in cornercase's terminals on {destination}. Its projects, \
         workspaces, tabs and splits come back, each tab with a new shell in its folder, and the window connects \
         again."
    );
    (title, message, RESTART)
}

fn update_job(destination: &str) -> Busy {
    Busy::new(format!("update-there:{destination}"), format!("Updating cornercase on {destination}…"))
}

fn restart_job(destination: &str) -> Busy {
    Busy::new(format!("restart-there:{destination}"), format!("Restarting cornercase on {destination}…"))
}

fn restart_args() -> Vec<String> {
    vec!["restart".to_string(), "--yes".to_string()]
}

fn plain_button(id: &'static str, icon: Icon, label: &str) -> Stateful<Div> {
    tall(ui::icon_text_button(id, icon, label.to_string(), Tone::Plain))
}

fn elsewhere_button() -> Stateful<Div> {
    plain_button("older-connect", Icon::Server, "Connect to another server")
        .tooltip(tip(with_keys("Connect to another server", Command::Connect)))
        .on_click(|_: &ClickEvent, window, cx| dialogs::connect(window, cx))
}

fn without_restart_button(cx: &mut Context<Root>) -> Stateful<Div> {
    plain_button("behind-connect", Icon::ArrowRight, "Connect without restarting").on_click(cx.listener(
        |this, _: &ClickEvent, _, cx| {
            this.updated_there = None;
            this.answered = None;
            this.start_again(cx);
        },
    ))
}

impl Root {
    pub(super) fn refused_page(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        match &self.conn {
            Conn::Older(older) => Some(self.older_page(older, cx)),
            Conn::Behind(behind) => Some(self.behind_page(behind, cx)),
            _ => None,
        }
    }

    fn older_page(&self, older: &Older, cx: &mut Context<Self>) -> Stateful<Div> {
        let busy = self.store.read(cx).busy(update_job(&older.destination).key());
        let buttons = row()
            .children(Self::update_button(older, busy, cx))
            .child(retry_button("older-retry", cx))
            .child(elsewhere_button());
        let content = column()
            .child(ui::emblem(Icon::Alert))
            .child(heading(&older_heading(older)))
            .child(lead_text(older_lead(older, Os::CURRENT)))
            .child(buttons)
            .child(answer_under(self.answer_for(update_job(&older.destination).key())));
        scrolled("older-page", content)
    }

    fn behind_page(&self, behind: &Behind, cx: &mut Context<Self>) -> Stateful<Div> {
        let busy = self.store.read(cx).busy(restart_job(&behind.destination).key());
        let buttons = row()
            .child(Self::restart_button(behind, busy, cx))
            .child(without_restart_button(cx))
            .child(elsewhere_button());
        let content = column()
            .child(ui::emblem(Icon::Refresh))
            .child(heading(&behind_heading(behind)))
            .child(lead_text(behind_lead(behind, Os::CURRENT)))
            .child(buttons)
            .child(answer_under(self.answer_for(restart_job(&behind.destination).key())));
        scrolled("behind-page", content)
    }

    fn update_button(older: &Older, busy: bool, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        cli::update_args(older)?;
        let label = format!("Update cornercase on {}…", older.destination);
        if busy {
            let busy_label = format!("Updating cornercase on {}…", older.destination);
            return Some(tall(ui::busy_button("older-update", busy_label, Tone::Primary)));
        }
        let older = older.clone();
        Some(
            tall(ui::icon_text_button("older-update", Icon::Download, label, Tone::Primary)).on_click(
                cx.listener(move |_, _: &ClickEvent, window, cx| Self::ask_update(older.clone(), window, cx)),
            ),
        )
    }

    fn restart_button(behind: &Behind, busy: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        if busy {
            let label = format!("Restarting cornercase on {}…", behind.destination);
            return tall(ui::busy_button("behind-restart", label, Tone::Danger));
        }
        let destination = behind.destination.clone();
        let label = format!("Restart cornercase on {destination}…");
        tall(ui::icon_text_button("behind-restart", Icon::Refresh, label, Tone::Danger))
            .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| Self::ask_restart(&destination, window, cx)))
    }

    fn ask_update(older: Older, window: &mut Window, cx: &mut Context<Self>) {
        let question = update_question(&older.destination, Os::CURRENT);
        Self::ask_then(question, move |this, cx| this.update_there(older.clone(), cx), window, cx);
    }

    fn update_there(&mut self, older: Older, cx: &mut Context<Self>) {
        let Some(args) = cli::update_args(&older) else { return };
        let job = update_job(&older.destination);
        let work = move || cli::answer_on(&Target::Local, &args);
        self.run_answered(job, work, move |this, done, cx| this.update_answered(older, done, cx), cx);
    }

    fn update_answered(&mut self, older: Older, done: bool, cx: &mut Context<Self>) {
        if done {
            self.updated_there = Some(older);
            self.start(cx);
        }
        cx.notify();
    }

    fn ask_restart(destination: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (question, name) = (restart_question(destination), destination.to_string());
        Self::ask_then(question, move |this, cx| this.restart_there(&name, cx), window, cx);
    }

    fn restart_there(&mut self, destination: &str, cx: &mut Context<Self>) {
        let target = cli::target();
        let work = move || cli::answer_on(&target, &restart_args());
        self.run_answered(restart_job(destination), work, Self::restart_answered, cx);
    }

    fn restart_answered(&mut self, done: bool, cx: &mut Context<Self>) {
        if done {
            self.answered = None;
            self.start(cx);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
