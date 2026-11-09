use std::path::PathBuf;

use gpui::{
    AnyElement, ClipboardItem, Context, Div, IntoElement as _, ParentElement as _, PathPromptOptions,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::{Snap, widgets};
use crate::appearance::Appearance;
use crate::dialogs::{self, Question};
use crate::panels::settings::SettingsPanel;
use crate::store::{Store, ToastKind};
use crate::theme::colours;
use crate::ui;

fn toast(text: impl Into<String>, kind: ToastKind, cx: &mut gpui::App) {
    let text = text.into();
    Store::get(cx).update(cx, |store, cx| store.toast(text, kind, cx));
}

fn export_text(cx: &gpui::App) -> anyhow::Result<(String, String)> {
    let appearance = Appearance::get(cx);
    let a = appearance.read(cx);
    Ok((a.in_use_id(), a.export_text()?))
}

impl SettingsPanel {
    fn duplicate_theme(&mut self, cx: &mut Context<Self>) {
        self.look.import_error = None;
        let name = Appearance::get(cx).update(cx, |appearance, cx| {
            let id = appearance.duplicate(cx);
            appearance.theme_of(&id).map(|theme| theme.name.clone())
        });
        toast(format!("Made {}", name.unwrap_or_default()), ToastKind::Done, cx);
    }

    fn copy_theme(cx: &mut Context<Self>) {
        match export_text(cx) {
            Ok((_, text)) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                toast("Copied the theme as JSON", ToastKind::Done, cx);
            }
            Err(why) => toast(format!("Could not copy the theme: {why:#}"), ToastKind::Error, cx),
        }
    }

    fn save_theme_file(cx: &mut Context<Self>) {
        let Ok((id, text)) = export_text(cx) else { return };
        let home = crate::desktop::home().unwrap_or_else(|| PathBuf::from("/"));
        let rx = cx.prompt_for_new_path(&home, Some(&format!("{id}.json")));
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                let done = crate::prefs::write_atomic(&path, &text);
                let _ = cx.update(|cx| match done {
                    Ok(()) => toast(format!("Saved {}", path.display()), ToastKind::Done, cx),
                    Err(why) => toast(format!("Could not save the theme: {why:#}"), ToastKind::Error, cx),
                });
            }
        })
        .detach();
    }

    fn import_theme_file(cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| this.imported_theme(&path, cx));
            }
        })
        .detach();
    }

    fn imported_theme(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        let stem = path.file_stem().map_or_else(|| "Imported".to_string(), |s| s.to_string_lossy().into_owned());
        let result = std::fs::read_to_string(path)
            .map_err(|why| anyhow::anyhow!("could not read {}: {why}", path.display()))
            .and_then(|text| Appearance::get(cx).update(cx, |appearance, cx| appearance.import(&text, &stem, cx)));
        match result {
            Ok(name) => {
                self.look.import_error = None;
                toast(format!("Imported {name}"), ToastKind::Done, cx);
            }
            Err(why) => self.look.import_error = Some(format!("{why:#}")),
        }
        cx.notify();
    }

    fn ask_delete(name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let panel = cx.entity();
        let question = Question::new(
            format!("Delete {name}?"),
            "Its file is removed from the themes folder.",
            "Delete",
            Vec::new(),
        )
        .on_yes(move |cx| {
            panel.update(cx, |_, cx| {
                Appearance::get(cx).update(cx, Appearance::delete);
            });
        });
        dialogs::confirm(question, window, cx);
    }

    fn share_buttons(snap: &Snap, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let name = snap.theme.name.clone();
        let mine = snap.mine;
        let folder = snap.clone_paths();
        let mut buttons = vec![
            ui::button("theme-duplicate", "Duplicate")
                .on_click(cx.listener(|this, _, _, cx| this.duplicate_theme(cx)))
                .into_any_element(),
            ui::button("theme-save", "Save to file…")
                .on_click(cx.listener(|_, _, _, cx| Self::save_theme_file(cx)))
                .into_any_element(),
            ui::button("theme-copy", "Copy as JSON")
                .on_click(cx.listener(|_, _, _, cx| Self::copy_theme(cx)))
                .into_any_element(),
            ui::button("theme-import", "Import…")
                .on_click(cx.listener(|_, _, _, cx| Self::import_theme_file(cx)))
                .into_any_element(),
            ui::button("theme-reveal", "Show in folder")
                .on_click(cx.listener(move |_, _, _, cx| Self::reveal_paths(&folder, cx)))
                .into_any_element(),
        ];
        if mine {
            buttons.push(Self::reset_button(cx));
            buttons.push(
                ui::danger_button("theme-delete", "Delete")
                    .on_click(cx.listener(move |_, _, window, cx| Self::ask_delete(&name, window, cx)))
                    .into_any_element(),
            );
        }
        buttons
    }

    fn reset_button(cx: &mut Context<Self>) -> AnyElement {
        ui::button("theme-reset", "Reset every change")
            .on_click(cx.listener(|_, _, _, cx| {
                Appearance::get(cx).update(cx, Appearance::reset_all);
            }))
            .into_any_element()
    }

    fn reveal_paths(paths: &(PathBuf, Option<PathBuf>), cx: &mut Context<Self>) {
        let (path, dir) = match &paths.1 {
            Some(file) => (file.clone(), false),
            None => (paths.0.clone(), true),
        };
        if let Err(why) = crate::desktop::reveal(&path, dir) {
            toast(format!("{why:#}"), ToastKind::Error, cx);
        }
    }

    pub(super) fn share_group(&self, snap: &Snap, cx: &mut Context<Self>) -> Div {
        let buttons = div().flex().flex_wrap().gap(px(8.)).py(px(14.)).children(Self::share_buttons(snap, cx));
        let error = self.look.import_error.clone().map(|why| div().pb(px(12.)).child(ui::error_line(why)));
        let caption =
            widgets::label("Share a theme", "A theme is one JSON file. Yours live beside the window's settings.");
        let body = widgets::card()
            .child(div().pt(px(12.)).text_color(colours::text()).child(caption))
            .child(buttons)
            .children(error);
        widgets::group("Share", body)
    }
}
