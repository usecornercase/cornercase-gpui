use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, InteractiveElement as _, IntoElement, MouseDownEvent,
    ParentElement as _, Pixels, Point, StatefulInteractiveElement as _, Styled as _, anchored, deferred, div, px,
};

use crate::platform::Os;
use crate::platform::paths::Style;
use crate::store::{self, ToastKind};
use crate::theme::{colours, radius};
use crate::ui;

use super::open::{self, Opener, Opening, opener_label};
use super::style::menu_entry;
use super::{FilesPanel, Menu, Target};

fn opens(opener: Opener, menu: &Menu) -> bool {
    opener != Opener::DefaultApp || menu.default_app
}

impl FilesPanel {
    pub(super) fn show_menu(&mut self, at: Point<Pixels>, target: Target, cx: &mut Context<Self>) {
        let Some(place) = self.place() else { return };
        let path = place.style.join(&place.root, &target.rel);
        let default_app = place.source.on_this_disk() && open::default_app_opens(Os::CURRENT, &path, target.dir);
        self.menu = Some(Menu { at, path, target, default_app });
        cx.notify();
    }

    fn open_with(&mut self, opening: Opening, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
        store::run_local(cx, opening.busy(), move || opening.run());
    }

    pub(super) fn copy(&mut self, text: String, what: &str, cx: &mut Context<Self>) {
        self.menu = None;
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.store.update(cx, |s, cx| s.toast(format!("Copied {what}"), ToastKind::Info, cx));
        cx.notify();
    }

    fn opener_entries(&self, menu: &Menu, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut entries: Vec<AnyElement> = Vec::new();
        let openers = self.store.read(cx).openers.clone();
        for (i, (opener, launch)) in openers.iter().enumerate() {
            if !opens(*opener, menu) {
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
        if openers.is_empty() {
            entries.push(div().px(px(10.)).py(px(4.)).child(ui::dim("No apps to open with")).into_any_element());
        }
        entries
    }

    fn copy_entries(menu: &Menu, cx: &mut Context<Self>) -> [AnyElement; 2] {
        let abs = menu.path.display().to_string();
        let rel = Style::of_path(&abs).shown(&menu.target.rel);
        [
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
        let mut entries = Vec::new();
        if self.place().is_some_and(|place| place.source.on_this_disk()) {
            entries = self.opener_entries(menu, cx);
            entries.push(div().my(px(4.)).h(px(1.)).bg(colours::border()).into_any_element());
        }
        entries.extend(Self::copy_entries(menu, cx));
        let frame = ui::glass::card(radius::card())
            .id("files-menu")
            .occlude()
            .min_w(px(200.))
            .p(px(4.))
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
