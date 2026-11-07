mod data;
mod form;
mod form_view;
mod rows;

use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, ClickEvent, Context, Div, Entity, Focusable as _, IntoElement, Render, ScrollAnchor,
    ScrollHandle, Window, div, prelude::*, px,
};

use self::data::{Ask, Doing};
use self::form::Draft;
use crate::cli;
use crate::dialogs::{self, Question};
use crate::model::machines::{self as model, Here, Machine, MachineIcon, MachineList};
use crate::panels::PanelKind;
use crate::panels::accounts::widgets;
use crate::platform::Os;
use crate::store::{self, Store, ToastKind, args};
use crate::ui::{self, Icon, TextField, TextFieldEvent, Tone};

const INTRO: &str = "Other computers your agents can run on. cornercase keeps an ssh link to each, and nothing there \
                     stops when the link drops.";
const EMPTY_HEADING: &str = "No machines yet";
const EMPTY_FOR: &str = "A machine is another computer your agents can run on, which cornercase links to through your \
                         own ssh.";
const RETRY_SHOWN: Duration = Duration::from_secs(2);

struct Running {
    name: String,
    doing: Doing,
}

struct Form {
    was: Option<Draft>,
    name: Entity<TextField>,
    destination: Entity<TextField>,
    folder: Entity<TextField>,
    icon: MachineIcon,
    tried: bool,
    saving: bool,
    error: Option<String>,
    hosts: Vec<String>,
}

impl Form {
    fn was_name(&self) -> Option<&str> {
        self.was.as_ref().map(|was| was.name.as_str())
    }

    fn draft(&self, cx: &App) -> Draft {
        let text = |field: &Entity<TextField>| field.read(cx).text().trim().to_string();
        Draft {
            name: text(&self.name),
            icon: self.icon,
            destination: text(&self.destination),
            folder: text(&self.folder),
        }
    }
}

pub(crate) struct MachinesPanel {
    here: Option<Here>,
    load_error: Option<String>,
    loading: bool,
    running: Vec<Running>,
    form: Option<Form>,
    reveal_form: bool,
    scroll: ScrollHandle,
    form_place: ScrollAnchor,
    was_open: bool,
}

fn toast_failure(error: &anyhow::Error, cx: &mut App) {
    let reason = cli::reason(error);
    Store::get(cx).update(cx, |s, cx| s.toast(reason, ToastKind::Error, cx));
}

impl MachinesPanel {
    fn new(cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        cx.observe(&store, |this, store, cx| this.store_changed(&store, cx)).detach();
        let scroll = ScrollHandle::new();
        let mut this = Self {
            here: None,
            load_error: None,
            loading: false,
            running: Vec::new(),
            form: None,
            reveal_form: false,
            form_place: ScrollAnchor::for_handle(scroll.clone()),
            scroll,
            was_open: true,
        };
        this.load(cx);
        this
    }

    fn form_field(placeholder: &'static str, text: String, cx: &mut Context<Self>) -> Entity<TextField> {
        let field = cx.new(|cx| TextField::new(placeholder, cx).in_tab_order());
        field.update(cx, |f, cx| f.set_text(text, cx));
        cx.subscribe(&field, |this, _, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Submit => this.submit_form(cx),
            TextFieldEvent::Cancel => this.close_form(cx),
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        field
    }

    fn store_changed(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let open = store.read(cx).sheet == Some(PanelKind::Machines);
        if open && !self.was_open {
            self.load(cx);
        }
        self.was_open = open;
        if open {
            cx.notify();
        }
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        cx.notify();
        store::run(cx, args(&["machines", "--json"]), |this, result, cx| {
            this.loading = false;
            match result.and_then(|out| model::parse_list(&out)) {
                Ok(list) => this.show(list, cx),
                Err(e) => this.load_error = Some(cli::reason(&e)),
            }
            cx.notify();
        });
    }

    fn show(&mut self, list: MachineList, cx: &mut Context<Self>) {
        self.load_error = None;
        self.here = Some(list.here);
        Store::get(cx).update(cx, |s, cx| s.take_machines(list.machines, cx));
    }

    fn answered(&mut self, out: &str, cx: &mut Context<Self>) {
        if let Ok(list) = model::parse_list(out) {
            self.show(list, cx);
        }
    }

    fn running_on(&self, name: &str) -> Option<Doing> {
        self.running.iter().find(|r| r.name == name).map(|r| r.doing)
    }

    fn settle(&mut self, name: &str, cx: &mut Context<Self>) {
        self.running.retain(|r| r.name != name);
        cx.notify();
    }

    fn act(&mut self, name: String, doing: Doing, cx: &mut Context<Self>) {
        if self.running_on(&name).is_some() {
            return;
        }
        if doing == Doing::Remove && self.form.as_ref().and_then(Form::was_name) == Some(name.as_str()) {
            self.form = None;
        }
        self.running.push(Running { name: name.clone(), doing });
        cx.notify();
        let (job, command) = (doing.job(&name), doing.command(&name));
        store::run_busy(cx, job, command, move |this, result, cx| match result {
            Ok(out) => {
                this.answered(&out, cx);
                if doing == Doing::Retry { Self::show_retry(name, cx) } else { this.settle(&name, cx) }
            }
            Err(e) => {
                toast_failure(&e, cx);
                this.settle(&name, cx);
            }
        });
    }

    fn show_retry(name: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RETRY_SHOWN).await;
            let _ = this.update(cx, |this, cx| this.settle(&name, cx));
        })
        .detach();
    }

    fn ask(ask: Ask, window: &mut Window, cx: &mut Context<Self>) {
        let (panel, name, doing) = (cx.entity(), ask.name, ask.doing);
        let question = Question::new(ask.title, ask.message, ask.button, Vec::new()).on_yes(move |cx| {
            let name = name.clone();
            panel.update(cx, |this, cx| this.act(name, doing, cx));
        });
        dialogs::confirm(question, window, cx);
    }

    fn open_form(&mut self, was: Option<Draft>, window: &mut Window, cx: &mut Context<Self>) {
        let draft = was.clone().unwrap_or_else(Draft::new);
        let name = Self::form_field("a name, such as vps", draft.name, cx);
        let destination = Self::form_field("user@host", draft.destination, cx);
        let folder = Self::form_field(form::DEFAULT_FOLDER, draft.folder, cx);
        let hosts = if cli::host().is_none() { dialogs::ssh_hosts() } else { Vec::new() };
        window.focus(&name.read(cx).focus_handle(cx));
        self.form = Some(Form {
            was,
            name,
            destination,
            folder,
            icon: draft.icon,
            tried: false,
            saving: false,
            error: None,
            hosts,
        });
        self.reveal_form = true;
        cx.notify();
    }

    fn close_form(&mut self, cx: &mut Context<Self>) {
        self.form = None;
        cx.notify();
    }

    fn submit_form(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.form.as_ref().filter(|form| !form.saving) else { return };
        let (was, draft) = (form.was.clone(), form.draft(cx));
        let refused = {
            let store = Store::get(cx);
            let machines = &store.read(cx).machines;
            let others = form::others(machines, was.as_ref().map(|was| was.name.as_str()));
            form::refusals(&draft, &others, true).any()
        };
        if refused {
            self.mark_form(|form| form.tried = true, cx);
            return;
        }
        match was.as_ref().map_or_else(|| Some(data::add(&draft)), |was| data::edit(was, &draft)) {
            Some(saving) => {
                self.mark_form(
                    |form| {
                        form.saving = true;
                        form.error = None;
                    },
                    cx,
                );
                store::run_busy(cx, saving.job, saving.command, Self::saved);
            }
            None => self.close_form(cx),
        }
    }

    fn mark_form(&mut self, mark: impl FnOnce(&mut Form), cx: &mut Context<Self>) {
        if let Some(form) = &mut self.form {
            mark(form);
        }
        cx.notify();
    }

    fn saved(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        match (result, &mut self.form) {
            (Ok(out), form) => {
                if form.as_ref().is_some_and(|form| form.saving) {
                    *form = None;
                }
                self.answered(&out, cx);
            }
            (Err(e), Some(form)) if form.saving => {
                form.saving = false;
                form.error = Some(cli::reason(&e));
            }
            (Err(e), _) => toast_failure(&e, cx),
        }
        cx.notify();
    }
}

impl MachinesPanel {
    fn add_button(tone: Tone, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        ui::icon_text_button("machines-add", Icon::Plus, "Add machine", tone)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_form(None, window, cx)))
    }

    fn adding(&self) -> bool {
        self.form.as_ref().is_some_and(|form| form.was.is_none())
    }

    fn top_bar(&self, cx: &mut Context<Self>) -> Div {
        let button = (!self.adding()).then(|| Self::add_button(Tone::Plain, cx));
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .min_h(px(30.))
            .child(div().flex_1().min_w_0().child(widgets::note(INTRO)))
            .children(button)
    }

    fn page(&self, machines: &[Machine], window: &Window, cx: &mut Context<Self>) -> Div {
        let failed = self.load_error.clone().map(|e| ui::error_line(format!("Could not read the list: {e}")));
        widgets::page()
            .child(self.top_bar(cx))
            .children(failed)
            .child(self.list_card(machines, window, cx))
            .child(widgets::hint(data::window_note(Os::CURRENT)))
    }

    fn empty_page(&self, cx: &mut Context<Self>) -> Div {
        let version = self.here.as_ref().map(|here| here.version.as_str());
        ui::emblem_page(Icon::Server, EMPTY_HEADING)
            .child(ui::emblem_line(EMPTY_FOR))
            .child(ui::emblem_line(form::empty_needs(version)))
            .child(div().pt(px(6.)).child(Self::add_button(Tone::Primary, cx)))
            .child(div().pt(px(6.)).child(widgets::hint(data::window_note(Os::CURRENT))))
    }

    fn waiting(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(error) = self.load_error.clone() else {
            return ui::loading("Listing the machines…").into_any_element();
        };
        let try_again = ui::button("machines-retry", "Try again")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.load(cx)));
        widgets::could_not_read(format!("Could not read the list: {error}"), try_again).into_any_element()
    }
}

impl Render for MachinesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.reveal_form) {
            self.form_place.scroll_to(window, cx);
            window.request_animation_frame();
        }
        let (machines, listed) = {
            let store = Store::get(cx);
            let s = store.read(cx);
            (s.machines.clone(), s.app.is_some() || self.here.is_some())
        };
        let body = if !listed {
            self.waiting(cx)
        } else if machines.is_empty() && self.form.is_none() {
            self.empty_page(cx).into_any_element()
        } else {
            self.page(&machines, window, cx).into_any_element()
        };
        div().id("machines").size_full().overflow_y_scroll().track_scroll(&self.scroll).child(body)
    }
}

pub(crate) fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(MachinesPanel::new).into()
}
