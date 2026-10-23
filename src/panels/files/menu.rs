use std::path::PathBuf;

use gpui::{
    AnyElement, AppContext as _, ClickEvent, ClipboardItem, Context, InteractiveElement as _, IntoElement,
    MouseDownEvent, ParentElement as _, Pixels, Point, StatefulInteractiveElement as _, Styled as _, anchored,
    deferred, div, px,
};

use crate::store::ToastKind;
use crate::theme::colours;
use crate::ui;

use super::open::{self, Launch, Opener};
use super::style::menu_entry;
use super::{FilesPanel, Menu, Target};

#[derive(Clone)]
struct Opening {
    opener: Opener,
    launch: Launch,
    path: PathBuf,
    line: Option<usize>,
    dir: bool,
}

impl Opening {
    fn argv(&self) -> Option<Vec<String>> {
        open::argv(self.opener, &self.launch, &self.path, self.line.map(|l| l + 1), self.dir)
    }

    fn run(&self) -> Result<(), String> {
        match self.argv() {
            Some(argv) => open::run(&argv),
            None => Err("not a file it opens".to_string()),
        }
    }
}

fn opener_label(opener: Opener, dir: bool) -> String {
    match opener {
        Opener::Finder if !dir => "Reveal in Finder".to_string(),
        Opener::DefaultApp => "Open with default app".to_string(),
        other => format!("Open in {}", other.label()),
    }
}

fn opens(opener: Opener, launch: &Launch, menu: &Menu) -> bool {
    opener != Opener::DefaultApp || open::argv(opener, launch, &menu.path, None, menu.target.dir).is_some()
}

impl FilesPanel {
    pub(super) fn show_menu(&mut self, at: Point<Pixels>, target: Target, cx: &mut Context<Self>) {
        let Some(place) = self.place() else { return };
        self.menu = Some(Menu { at, path: place.root.join(&target.rel), target });
        cx.notify();
    }

    fn open_with(&mut self, opening: Opening, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
        let label = opening.opener.label();
        let job = cx.background_spawn(async move { opening.run() });
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            let Err(e) = job.await else { return };
            let text = format!("could not open in {label}: {e}");
            let _ = store.update(cx, |s, cx| s.toast(text, ToastKind::Error, cx));
        })
        .detach();
    }

    pub(super) fn copy(&mut self, text: String, what: &str, cx: &mut Context<Self>) {
        self.menu = None;
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.store.update(cx, |s, cx| s.toast(format!("Copied {what}"), ToastKind::Info, cx));
        cx.notify();
    }

    fn opener_entries(&self, menu: &Menu, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut entries: Vec<AnyElement> = Vec::new();
        for (i, (opener, launch)) in self.openers.iter().enumerate() {
            if !opens(*opener, launch, menu) {
                continue;
            }
            let opening = Opening {
                opener: *opener,
                launch: launch.clone(),
                path: menu.path.clone(),
                line: menu.target.line,
                dir: menu.target.dir,
            };
            let entry = menu_entry(("files-menu-open", i), opener_label(*opener, menu.target.dir))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.open_with(opening.clone(), cx)));
            entries.push(entry.into_any_element());
        }
        if self.openers.is_empty() {
            entries.push(div().px(px(10.)).py(px(4.)).child(ui::dim("No apps to open with")).into_any_element());
        }
        entries
    }

    fn copy_entries(menu: &Menu, cx: &mut Context<Self>) -> [AnyElement; 3] {
        let abs = menu.path.display().to_string();
        let rel = menu.target.rel.display().to_string();
        [
            div().my(px(4.)).h(px(1.)).bg(colours::border()).into_any_element(),
            menu_entry("files-menu-copy", "Copy path")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.copy(abs.clone(), "path", cx)))
                .into_any_element(),
            menu_entry("files-menu-copy-rel", "Copy relative path")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.copy(rel.clone(), "relative path", cx)))
                .into_any_element(),
        ]
    }

    pub(super) fn render_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.menu.as_ref()?;
        let mut entries = self.opener_entries(menu, cx);
        entries.extend(Self::copy_entries(menu, cx));
        let frame = div()
            .id("files-menu")
            .min_w(px(200.))
            .p(px(4.))
            .rounded(px(6.))
            .bg(colours::panel())
            .border_1()
            .border_color(colours::border())
            .shadow_lg()
            .text_size(px(13.))
            .children(entries)
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                this.menu = None;
                cx.notify();
            }));
        Some(
            deferred(anchored().position(menu.at).snap_to_window_with_margin(px(8.)).child(frame))
                .with_priority(1)
                .into_any_element(),
        )
    }
}
