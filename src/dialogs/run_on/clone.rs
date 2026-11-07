use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};

use crate::cli::{self, Target};
use crate::dialogs::{body_height, frame, muted};
use crate::model::machines::run_on::clone::{
    FileList, IgnoredFile, NO_FILES, READING, clone_address, destination, ignored_args, intro, not_offered,
};
use crate::notify::tip;
use crate::platform::Os;
use crate::store::{self, Busy, CloneThere, Store};
use crate::theme::colours;
use crate::ui::{self, TICK_BOX, TextField, TextFieldEvent};

const NO_ADDRESS: &str = "Give the address of the repository.";
const ROW_GAP: f32 = 8.;
const CUT_FROM: usize = 50;

pub(super) struct CloneForm {
    place: Place,
    url: Entity<TextField>,
    files: FileList,
    error: Option<String>,
}

struct Place {
    machine: String,
    project: u64,
    name: String,
    from: String,
    folder: String,
}

fn back(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

pub(super) fn open(machine: &str, project: Option<u64>, back: FocusHandle, cx: &mut App) {
    let store = Store::get(cx);
    let s = store.read(cx);
    let Some(ours) = s.project_here(project) else { return };
    let listed = s.machines.iter().find(|listed| listed.name == machine);
    let address = clone_address(&ours.remotes).unwrap_or_default().to_string();
    let place = Place {
        machine: machine.to_string(),
        project: ours.id,
        name: ours.name.clone(),
        from: ours.path.clone(),
        folder: listed.map(|listed| listed.folder.clone()).unwrap_or_default(),
    };
    let view = cx.new(|cx| CloneForm::new(place, address, cx));
    let focus = view.read(cx).url.read(cx).focus_handle(cx);
    store.update(cx, |s, cx| s.show_dialog_over(view.into(), focus, back, cx));
}

fn list_height(window: f32) -> f32 {
    (body_height(window) - 200.).clamp(90., 220.)
}

fn no_box() -> Div {
    div().w(px(TICK_BOX)).flex_none()
}

fn file_row(at: usize, file: &IgnoredFile, ticked: bool, cx: &Context<CloneForm>) -> AnyElement {
    let path = div().flex_1().min_w_0().child(ui::one_line(file.path.clone()));
    let size = div().flex_none().text_color(colours::muted()).child(file.size_words());
    let row = div()
        .id(("clone-file", at))
        .flex()
        .items_center()
        .gap(px(ROW_GAP))
        .text_size(px(12.5))
        .when(file.path.chars().count() > CUT_FROM, |row| row.tooltip(tip(file.path.clone())));
    if file.too_big {
        return row.text_color(colours::muted()).child(no_box()).child(path).child(size).into_any_element();
    }
    row.cursor_pointer()
        .child(ui::tick_box(ticked))
        .child(path)
        .child(size)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(at, cx)))
        .into_any_element()
}

fn file_rows(files: &[(IgnoredFile, bool)], left_out: usize, height: f32, cx: &Context<CloneForm>) -> Div {
    let rows = files.iter().enumerate().map(|(at, (file, ticked))| file_row(at, file, *ticked, cx));
    let more = (left_out > 0).then(|| muted(not_offered(left_out)));
    let list = div()
        .id("clone-files")
        .max_h(px(height))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(6.))
        .children(rows)
        .children(more);
    div().child(list)
}

impl CloneForm {
    fn new(place: Place, address: String, cx: &mut Context<Self>) -> Self {
        let url = cx.new(|cx| {
            let mut field = TextField::new("The repository's address", cx).in_tab_order().wraps();
            field.set_text(address, cx);
            field
        });
        cx.subscribe(&url, |this: &mut Self, _, event, cx| this.on_field(event, cx)).detach();
        let busy = Busy::new(format!("clone-files:{}", cx.entity_id()), READING);
        store::run_on_busy(cx, busy, Target::Local, ignored_args(&place.from), |this: &mut Self, result, cx| {
            this.files = FileList::heard(result.map_err(|e| cli::reason(&e)));
            cx.notify();
        });
        Self { place, url, files: FileList::Reading, error: None }
    }

    fn on_field(&mut self, event: &TextFieldEvent, cx: &mut Context<Self>) {
        match event {
            TextFieldEvent::Submit => self.submit(self.files.ticked(), cx),
            TextFieldEvent::Cancel => back(cx),
            TextFieldEvent::Changed => {
                self.error = None;
                cx.notify();
            }
        }
    }

    fn toggle(&mut self, at: usize, cx: &mut Context<Self>) {
        self.error = self.files.toggle(at).err();
        cx.notify();
    }

    fn submit(&mut self, files: Vec<String>, cx: &mut Context<Self>) {
        let url = self.url.read(cx).text().trim().to_string();
        if url.is_empty() {
            self.error = Some(NO_ADDRESS.to_string());
            cx.notify();
            return;
        }
        let place = &self.place;
        let clone = CloneThere {
            machine: place.machine.clone(),
            project: place.project,
            name: place.name.clone(),
            url,
            folder: place.folder.clone(),
            from: place.from.clone(),
            files,
        };
        Store::get(cx).update(cx, |s, cx| {
            s.close_dialog(cx);
            s.clone_there(clone, cx);
        });
    }

    fn files_part(&self, height: f32, cx: &Context<Self>) -> Div {
        match &self.files {
            FileList::Reading => ui::busy("clone-reading", READING),
            FileList::Failed(why) => ui::error_line(why.clone()),
            FileList::Read { files, left_out: 0 } if files.is_empty() => muted(NO_FILES),
            FileList::Read { files, left_out } => file_rows(files, *left_out, height, cx),
        }
    }

    fn body(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let place = &self.place;
        let into = destination(&place.folder, &self.url.read(cx).text());
        let height = list_height(f32::from(window.viewport_size().height));
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(muted(intro(&place.machine, Os::CURRENT)))
            .child(ui::field("Address", ui::boxed(&self.url, window, cx)))
            .child(ui::field(
                format!("Folder on {}", place.machine),
                div().text_size(px(12.5)).text_color(colours::soft()).child(into),
            ))
            .child(ui::field("Files to copy into the clone", self.files_part(height, cx)))
            .children(self.error.clone().map(ui::error_line))
    }

    fn buttons(&self, cx: &Context<Self>) -> Div {
        let skip = self.files.offers_any().then(|| {
            ui::button("clone-skip", "Skip files")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(Vec::new(), cx)))
        });
        div()
            .flex()
            .gap(px(ROW_GAP))
            .child(ui::button("clone-back", "Back").on_click(|_: &ClickEvent, _, cx| back(cx)))
            .children(skip)
            .child(
                ui::primary_button("clone-go", "Clone")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(this.files.ticked(), cx))),
            )
    }
}

impl Render for CloneForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = self.body(window, cx);
        frame(format!("Clone {} on {}", self.place.name, self.place.machine), body, self.buttons(cx))
    }
}
