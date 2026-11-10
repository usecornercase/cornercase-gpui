use std::path::{Path, PathBuf};

use gpui::{
    AppContext as _, ClickEvent, Context, Div, ParentElement as _, PathPromptOptions, Stateful,
    StatefulInteractiveElement as _,
};

use super::no_tab::{answer_under, column, heading, lead_text, retry_button, row, scrolled, tall};
use crate::app::{Conn, Root};
use crate::cli::{self, Binary, Missing, Origin, Problem};
use crate::desktop;
use crate::platform::{Os, paths::home_relative};
use crate::store::Busy;
use crate::ui::{self, Icon, Tone};

const UPDATE_HERE: &str = "update-here";

fn shown(path: &Path, home: &str) -> String {
    home_relative(&path.to_string_lossy(), home)
}

fn heading_of(missing: &Missing) -> String {
    match missing {
        Missing::NotFound { .. }
        | Missing::Unusable { problem: Problem::NotAProgram, origin: Origin::Variable, .. } => {
            "cornercase was not found".to_string()
        }
        Missing::Unusable { problem: Problem::NotAProgram, .. } => "That is not a program".to_string(),
        Missing::Unusable { problem: Problem::TooOld(version), .. } => format!("cornercase {version} is too old"),
        Missing::Unusable { problem: Problem::DoesNotStart(_), .. } => "cornercase does not start".to_string(),
        Missing::Unusable { problem: Problem::NotCornercase(_), .. } => "That is not cornercase".to_string(),
    }
}

fn origin_note(origin: Origin) -> &'static str {
    match origin {
        Origin::Variable => " (CORNERCASE_BIN)",
        Origin::Chosen => " (chosen by hand)",
        Origin::Path | Origin::LoginShell | Origin::Installer => "",
    }
}

fn not_found(places: &[PathBuf], asked_shell: bool, os: Os, home: &str) -> String {
    let shell = if asked_shell { ", in your login shell's PATH" } else { "" };
    let places: Vec<String> = places.iter().map(|place| shown(place, home)).collect();
    format!(
        "This window needs cornercase {} or newer on this {}, and found none on the PATH{shell} or where its \
         installers put it: {}. Install it, then try again, or choose it yourself.",
        cli::MINIMUM,
        desktop::machine(os),
        places.join(", ")
    )
}

fn unusable(path: &Path, origin: Origin, problem: &Problem, home: &str) -> String {
    let (place, min) = (shown(path, home), cli::MINIMUM);
    let named = format!("{place}{}", origin_note(origin));
    let text = match (problem, origin) {
        (Problem::NotAProgram, Origin::Variable) => format!(
            "CORNERCASE_BIN is set to {place}, which is not a program here. Point it at cornercase {min} or newer, \
             or unset it and restart the window."
        ),
        (Problem::NotAProgram, _) => format!(
            "{place} cannot be run. Choose cornercase itself, the file named cornercase in a folder such as \
             ~/.local/bin."
        ),
        (Problem::TooOld(version), _) => format!(
            "This window needs cornercase {min} or newer; {named} is {version}. Update it here, or as the installation \
             guide says, then try again."
        ),
        (Problem::DoesNotStart(why), _) => format!("{named} could not be started: {why}."),
        (Problem::NotCornercase(answer), _) if answer.is_empty() => {
            format!("{named} printed nothing for `--version`.")
        }
        (Problem::NotCornercase(answer), _) => {
            format!("{named} answered `--version` with \"{answer}\", not with cornercase's version.")
        }
    };
    if origin == Origin::Variable && *problem != Problem::NotAProgram {
        return format!("{text} CORNERCASE_BIN decides which cornercase this window runs.");
    }
    text
}

fn lead(missing: &Missing, os: Os, home: &str) -> String {
    match missing {
        Missing::NotFound { places, asked_shell } => not_found(places, *asked_shell, os, home),
        Missing::Unusable { path, origin, problem } => unusable(path, *origin, problem, home),
    }
}

fn offers_choice(missing: &Missing) -> bool {
    !matches!(missing, Missing::Unusable { origin: Origin::Variable, .. })
}

fn emblem_of(missing: &Missing) -> Icon {
    if missing.label() == "not found" { Icon::Download } else { Icon::Alert }
}

fn too_old_path(missing: &Missing) -> Option<&Path> {
    match missing {
        Missing::Unusable { path, problem: Problem::TooOld(_), .. } => Some(path),
        _ => None,
    }
}

fn update_question(path: &Path, os: Os, home: &str) -> (String, String, &'static str) {
    let machine = desktop::machine(os);
    let message = format!(
        "This runs `cornercase update` with {}, which installs the latest release in its place, or says which \
         command updates it when it cannot (Homebrew, a folder it cannot write). A server already running keeps its \
         release until it restarts.",
        shown(path, home)
    );
    (format!("Update cornercase on this {machine}?"), message, "Update")
}

fn update_job(os: Os) -> Busy {
    Busy::new(UPDATE_HERE, format!("Updating cornercase on this {}…", desktop::machine(os)))
}

fn guide_button(tone: Tone, cx: &mut Context<Root>) -> Stateful<Div> {
    tall(ui::icon_text_button("missing-guide", Icon::External, "Installation guide", tone))
        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.open_url(cli::INSTALL_GUIDE)))
}

fn choose_button(cx: &mut Context<Root>) -> Stateful<Div> {
    tall(ui::icon_text_button("missing-choose", Icon::FolderOpen, "Choose cornercase…", Tone::Plain))
        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| Root::choose_cornercase(cx)))
}

enum Choice {
    Use(Binary, String),
    Stay(Missing),
}

fn choice(checked: Result<Binary, Missing>) -> Choice {
    match checked {
        Ok(binary) => match binary.path.to_str().map(str::to_string) {
            Some(saved) => Choice::Use(binary, saved),
            None => Choice::Stay(Missing::Unusable {
                path: binary.path,
                origin: Origin::Chosen,
                problem: Problem::DoesNotStart("its path is not valid text, so it cannot be kept".to_string()),
            }),
        },
        Err(missing) => Choice::Stay(missing),
    }
}

impl Root {
    pub(super) fn missing_page(&self, missing: &Missing, cx: &mut Context<Self>) -> Stateful<Div> {
        let lead = lead(missing, Os::CURRENT, &desktop::home_text());
        let update = too_old_path(missing).map(|path| self.update_here_button(path, cx));
        let tone = if update.is_some() { Tone::Plain } else { Tone::Primary };
        let buttons = row()
            .children(update)
            .child(guide_button(tone, cx))
            .child(retry_button("missing-retry", cx))
            .children(offers_choice(missing).then(|| choose_button(cx)));
        let content = column()
            .child(ui::emblem(emblem_of(missing)))
            .child(heading(&heading_of(missing)))
            .child(lead_text(lead))
            .child(buttons)
            .child(answer_under(self.answer_for(UPDATE_HERE)));
        scrolled("missing-page", content)
    }

    fn update_here_button(&self, path: &Path, cx: &mut Context<Self>) -> Stateful<Div> {
        if self.store.read(cx).busy(UPDATE_HERE) {
            let label = format!("Updating cornercase on this {}…", desktop::machine(Os::CURRENT));
            return tall(ui::busy_button("missing-update", label, Tone::Primary));
        }
        let path = path.to_path_buf();
        tall(ui::icon_text_button("missing-update", Icon::Download, "Update cornercase…", Tone::Primary)).on_click(
            cx.listener(move |_, _: &ClickEvent, window, cx| {
                let question = update_question(&path, Os::CURRENT, &desktop::home_text());
                let path = path.clone();
                Self::ask_then(question, move |this, cx| this.update_here(path.clone(), cx), window, cx);
            }),
        )
    }

    fn update_here(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let work = move || cli::answer_at(&path, &["update"]);
        self.run_answered(update_job(Os::CURRENT), work, Self::update_here_answered, cx);
    }

    fn update_here_answered(&mut self, done: bool, cx: &mut Context<Self>) {
        if done {
            self.start(cx);
        }
        cx.notify();
    }

    fn choose_cornercase(cx: &mut Context<Self>) {
        let options =
            PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Use".into()) };
        let picked = cx.prompt_for_paths(options);
        cx.spawn(async move |this, cx| {
            let _ = match picked.await {
                Ok(Ok(Some(paths))) => {
                    let first = paths.into_iter().next();
                    first.map(|path| this.update(cx, |this, cx| this.check_chosen(path, cx)))
                }
                Ok(Err(e)) => {
                    let why = format!("The file picker could not open: {e}");
                    Some(this.update(cx, |this, cx| this.say(&why, cx)))
                }
                _ => None,
            };
        })
        .detach();
    }

    fn check_chosen(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.conn = Conn::Starting;
        cx.notify();
        let checking = cx.background_spawn(async move { cli::check_chosen(path) });
        cx.spawn(async move |this, cx| {
            let checked = checking.await;
            let _ = this.update(cx, |this, cx| this.chosen(checked, cx));
        })
        .detach();
    }

    fn chosen(&mut self, checked: Result<Binary, Missing>, cx: &mut Context<Self>) {
        match choice(checked) {
            Choice::Use(binary, saved) => {
                if let Err(e) = self.store.update(cx, |s, cx| s.set_pref("cornercase_bin", &saved, cx)) {
                    self.say(&e.to_string(), cx);
                }
                cli::use_binary(binary);
                self.start(cx);
            }
            Choice::Stay(missing) => self.conn = Conn::Missing(missing),
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
