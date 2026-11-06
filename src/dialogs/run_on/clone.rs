use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::dialogs::frame;
use crate::model::machines::run_on::clone::{FILES_NOTE, clone_address, destination};
use crate::store::{CloneThere, Store};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

const NO_ADDRESS: &str = "Give the address of the repository.";

pub(super) struct CloneForm {
    machine: String,
    project: u64,
    name: String,
    folder: String,
    url: Entity<TextField>,
    error: Option<String>,
}

fn back(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

pub(super) fn open(machine: &str, project: Option<u64>, back: FocusHandle, cx: &mut App) {
    let store = Store::get(cx);
    let s = store.read(cx);
    let Some(ours) = s.project_here(project) else { return };
    let listed = s.machines.iter().find(|listed| listed.name == machine);
    let folder = listed.map(|listed| listed.folder.clone()).unwrap_or_default();
    let address = clone_address(&ours.remotes).unwrap_or_default().to_string();
    let (id, name) = (ours.id, ours.name.clone());
    let machine = machine.to_string();
    let view = cx.new(|cx| CloneForm::new((machine, id, name), folder, &address, cx));
    let focus = view.read(cx).url.read(cx).focus_handle(cx);
    store.update(cx, |s, cx| s.show_dialog_over(view.into(), focus, back, cx));
}

impl CloneForm {
    fn new(
        (machine, project, name): (String, u64, String),
        folder: String,
        address: &str,
        cx: &mut Context<Self>,
    ) -> Self {
        let address = address.to_string();
        let url = cx.new(|cx| {
            let mut field = TextField::new("The repository's address", cx).in_tab_order().wraps();
            field.set_text(address, cx);
            field
        });
        cx.subscribe(&url, |this: &mut Self, _, event, cx| this.on_field(event, cx)).detach();
        Self { machine, project, name, folder, url, error: None }
    }

    fn on_field(&mut self, event: &TextFieldEvent, cx: &mut Context<Self>) {
        match event {
            TextFieldEvent::Submit => self.submit(cx),
            TextFieldEvent::Cancel => back(cx),
            TextFieldEvent::Changed => {
                self.error = None;
                cx.notify();
            }
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let url = self.url.read(cx).text().trim().to_string();
        if url.is_empty() {
            self.error = Some(NO_ADDRESS.to_string());
            cx.notify();
            return;
        }
        let clone = CloneThere {
            machine: self.machine.clone(),
            project: self.project,
            name: self.name.clone(),
            url,
            folder: self.folder.clone(),
        };
        Store::get(cx).update(cx, |s, cx| {
            s.close_dialog(cx);
            s.clone_there(clone, cx);
        });
    }
}

impl Render for CloneForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let into = destination(&self.folder, &self.url.read(cx).text());
        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::field("Address", ui::boxed(&self.url, window, cx)))
            .child(ui::field(
                format!("Folder on {}", self.machine),
                div().text_size(px(12.5)).text_color(colours::soft()).child(into),
            ))
            .child(ui::dim(FILES_NOTE))
            .children(self.error.clone().map(ui::error_line));
        let buttons = div()
            .flex()
            .gap(px(8.))
            .child(ui::button("clone-back", "Back").on_click(|_: &ClickEvent, _, cx| back(cx)))
            .child(
                ui::primary_button("clone-go", "Clone")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
            );
        frame(format!("Clone {} on {}", self.name, self.machine), body, buttons)
    }
}
