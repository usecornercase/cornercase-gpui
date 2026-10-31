use gpui::{Context, Div, div, prelude::*, px};

use super::changes::EXPORTING;
use super::data::Exported;
use super::{SettingsPanel, widgets};
use crate::store::Store;
use crate::ui::{self, Icon, Tone};

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn export_summary(e: Exported) -> Div {
    let went = format!(
        "The last export held {} and {}.",
        plural(e.settings, "setting", "settings"),
        plural(e.repositories, "repository", "repositories")
    );
    let left_out = (!e.left_out.is_empty())
        .then(|| ui::dim(format!("Left out, as they are personal: {}.", e.left_out.join(", "))).text_size(px(11.5)));
    div()
        .flex()
        .flex_col()
        .gap(px(5.))
        .pb(px(12.))
        .child(ui::done_line(went))
        .children(left_out)
        .children(e.warnings.into_iter().map(ui::warning_line))
}

impl SettingsPanel {
    pub(super) fn render_share(&self, cx: &mut Context<Self>) -> Vec<Div> {
        vec![widgets::group("Export", self.export_card(cx)), widgets::group("Import", self.import_card(cx))]
    }

    fn export_buttons(cx: &mut Context<Self>) -> Div {
        let buttons = div().flex().items_center().gap(px(8.));
        if Store::get(cx).read(cx).busy(EXPORTING) {
            return buttons.child(ui::busy_button("export-copy", "Exporting…", Tone::Primary));
        }
        buttons
            .child(
                ui::icon_text_button("export-file", Icon::Download, "Save to file…", Tone::Plain)
                    .on_click(cx.listener(|_, _, _, cx| Self::export_to_file(cx))),
            )
            .child(
                ui::icon_text_button("export-copy", Icon::Copy, "Copy", Tone::Primary)
                    .on_click(cx.listener(|_, _, _, cx| Self::export(None, cx))),
            )
    }

    fn export_card(&self, cx: &mut Context<Self>) -> Div {
        let label = widgets::label(
            "Your setup, as TOML",
            "Agents, issues, worktrees, the interface and each repository's commands. Never tokens, accounts or \
             personal keys (Jira email, remote access, keep awake).",
        );
        widgets::card()
            .child(widgets::row(true, label, Self::export_buttons(cx), div()))
            .children(self.exported.clone().map(export_summary))
    }

    fn import_card(&self, cx: &mut Context<Self>) -> Div {
        let choose = ui::icon_text_button("import", Icon::FolderOpen, "Choose a file…", Tone::Plain)
            .on_click(cx.listener(|_, _, _, cx| Self::choose_import(cx)));
        let label = widgets::label(
            "A setup a colleague shared",
            "You see what would change, and nothing changes until you import it.",
        );
        let card = widgets::card().child(widgets::row(true, label, choose, div()));
        match &self.import {
            Some(import) => card.child(Self::import_preview(import, cx)),
            None => card,
        }
    }
}
