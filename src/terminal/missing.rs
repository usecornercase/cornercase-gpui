use std::path::{Path, PathBuf};

use gpui::{
    AppContext as _, ClickEvent, Context, Div, ParentElement as _, PathPromptOptions, Stateful,
    StatefulInteractiveElement as _,
};

use super::no_tab::{heading, lead_text, page, row, tall};
use crate::app::{Conn, Root};
use crate::cli::{self, Binary, Missing, Origin, Problem};
use crate::desktop;
use crate::platform::{Os, paths::home_relative};
use crate::ui::{self, Icon, Tone};

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
        cli::minimum(os),
        desktop::machine(os),
        places.join(", ")
    )
}

fn unusable(path: &Path, origin: Origin, problem: &Problem, os: Os, home: &str) -> String {
    let (place, min) = (shown(path, home), cli::minimum(os));
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
            "This window needs cornercase {min} or newer; {named} is {version}. Update it as the installation guide \
             says, then try again."
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
        Missing::Unusable { path, origin, problem } => unusable(path, *origin, problem, os, home),
    }
}

fn offers_choice(missing: &Missing) -> bool {
    !matches!(missing, Missing::Unusable { origin: Origin::Variable, .. })
}

fn emblem_of(missing: &Missing) -> Icon {
    if missing.label() == "not found" { Icon::Download } else { Icon::Alert }
}

fn guide_button(cx: &mut Context<Root>) -> Stateful<Div> {
    tall(ui::icon_text_button("missing-guide", Icon::External, "Installation guide", Tone::Primary))
        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.open_url(cli::INSTALL_GUIDE)))
}

fn retry_button(cx: &mut Context<Root>) -> Stateful<Div> {
    tall(ui::icon_text_button("missing-retry", Icon::Refresh, "Try again", Tone::Plain))
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.restart(cx)))
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
        Ok(binary) => {
            let saved = binary.path.to_string_lossy().into_owned();
            Choice::Use(binary, saved)
        }
        Err(missing) => Choice::Stay(missing),
    }
}

impl Root {
    pub(super) fn missing_page(missing: &Missing, cx: &mut Context<Self>) -> Div {
        let lead = lead(missing, Os::CURRENT, &desktop::home_text());
        let buttons = row()
            .child(guide_button(cx))
            .child(retry_button(cx))
            .children(offers_choice(missing).then(|| choose_button(cx)));
        page()
            .child(ui::emblem(emblem_of(missing)))
            .child(heading(&heading_of(missing)))
            .child(lead_text(lead))
            .child(buttons)
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        self.start(cx);
        cx.notify();
    }

    fn choose_cornercase(cx: &mut Context<Self>) {
        let options =
            PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Use".into()) };
        let picked = cx.prompt_for_paths(options);
        cx.spawn(async move |this, cx| {
            let path = picked.await.ok().and_then(Result::ok).flatten().and_then(|paths| paths.into_iter().next());
            if let Some(path) = path {
                let _ = this.update(cx, |this, cx| this.check_chosen(path, cx));
            }
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
