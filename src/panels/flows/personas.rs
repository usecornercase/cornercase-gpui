mod form;
mod import;

use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, WeakEntity,
    Window, div, prelude::FluentBuilder as _, px,
};

use self::form::PersonaForm;
use super::FlowsPanel;
use super::actions::toast;
use super::library::{self, Draft, Library, Persona, Unreadable, followers, kib, parse_shown};
use super::model::FlowInfo;
use super::widgets::{error_small, error_text};
use crate::cli::{self, Feature};
use crate::dialogs::{self, Question};
use crate::platform::Os;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, Icon, Tone};

const LIST_WIDTH: f32 = 320.;
const EMPTY: &str = "No personas yet. New writes one of yours; Import… brings them from a repository, an address or \
                     a folder, such as msitarzewski/agency-agents.";
const HOW: &str = "How a role's agent works, picked in a role's Persona row of a flow.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pick {
    Persona(String),
    Broken(String),
}

pub(super) struct PersonasPage {
    panel: WeakEntity<FlowsPanel>,
    library: Option<Result<Library, String>>,
    flows: Vec<FlowInfo>,
    selected: Option<Pick>,
    form: Option<Entity<PersonaForm>>,
    opening: Option<String>,
}

impl FlowsPanel {
    pub(super) fn open_personas(&mut self, cx: &mut Context<Self>) {
        let panel = cx.entity().downgrade();
        let flows = self.listing.as_ref().map(|listing| listing.flows.clone()).unwrap_or_default();
        let library = self.library.clone().map(Ok);
        let page = cx.new(|cx| {
            let mut page = PersonasPage { panel, library, flows, selected: None, form: None, opening: None };
            page.ensure_selection();
            PersonasPage::reload(cx);
            page
        });
        self.personas = Some(page);
        cx.notify();
    }

    pub(super) fn close_personas(&mut self, cx: &mut Context<Self>) {
        self.personas = None;
        cx.notify();
    }

    pub(super) fn follow_flows_in_personas(&self, cx: &mut Context<Self>) {
        let (Some(page), Some(listing)) = (&self.personas, &self.listing) else { return };
        let flows = listing.flows.clone();
        page.update(cx, |page, cx| {
            if page.flows != flows {
                page.flows = flows;
                cx.notify();
            }
        });
    }
}

impl PersonasPage {
    pub(super) fn reload(cx: &mut Context<Self>) {
        if Store::get(cx).read(cx).releases().lacks(Feature::FlowPersonas).is_some() {
            return;
        }
        store::run(cx, args(&library::LIST), |this, result, cx| {
            let read = result.and_then(|out| library::parse_library(&out));
            if let Ok(library) = &read {
                let library = library.clone();
                let _ = this.panel.update(cx, |panel, cx| panel.library_read(library, cx));
            }
            this.library = Some(read.map_err(|e| cli::reason(&e)));
            this.ensure_selection();
            cx.notify();
        });
    }

    fn library(&self) -> Option<&Library> {
        self.library.as_ref()?.as_ref().ok()
    }

    fn ensure_selection(&mut self) {
        let Some(library) = self.library() else { return };
        let exists = match &self.selected {
            Some(Pick::Persona(id)) => library.get(id).is_some(),
            Some(Pick::Broken(path)) => library.broken.iter().any(|broken| broken.path == *path),
            None => false,
        };
        if !exists {
            self.selected = library.personas.first().map(|persona| Pick::Persona(persona.id.clone()));
        }
    }

    pub(super) fn form_closed(&mut self, saved: Option<String>, cx: &mut Context<Self>) {
        self.form = None;
        if let Some(id) = saved {
            self.selected = Some(Pick::Persona(id));
            Self::reload(cx);
        }
        cx.notify();
    }

    fn open_form(&mut self, editing: Option<String>, draft: Draft, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        self.form = Some(cx.new(|cx| PersonaForm::new(page, (editing, draft), window, cx)));
        cx.notify();
    }

    fn edit(&mut self, id: String, cx: &mut Context<Self>) {
        if self.opening.is_some() {
            return;
        }
        self.opening = Some(id.clone());
        cx.notify();
        store::run(cx, library::shown_args(&id), move |this, result, cx| {
            this.opening = None;
            match result.and_then(|out| parse_shown(&out)) {
                Ok(shown) => Self::open_edit(id, Draft::of(&shown), cx),
                Err(e) => toast(cli::reason(&e), ToastKind::Error, cx),
            }
            cx.notify();
        });
    }

    fn open_edit(id: String, draft: Draft, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        let Some(window) = cx.active_window() else { return };
        let _ = window.update(cx, |_, window, cx| {
            let _ = page.update(cx, |page, cx| page.open_form(Some(id), draft, window, cx));
        });
    }

    fn remove(&self, id: &str, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        let followed = followers(&self.flows, id);
        let question =
            Question::new(format!("Remove {name}?"), removal(name, &followed), "Remove", library::remove_args(id))
                .on_reply(move |result, cx| {
                    match result {
                        Ok(_) => toast("Removed the persona", ToastKind::Done, cx),
                        Err(e) => toast(cli::reason(&e), ToastKind::Error, cx),
                    }
                    let _ = page.update(cx, |_, cx| PersonasPage::reload(cx));
                });
        dialogs::confirm(question, window, cx);
    }

    fn back(&self, cx: &mut Context<Self>) {
        let _ = self.panel.update(cx, FlowsPanel::close_personas);
    }
}

fn removal(name: &str, followed: &[String]) -> String {
    let after = "Running flows keep their own copy.";
    if followed.is_empty() {
        return format!("{name} goes from your personas; no flow follows it. {after}");
    }
    format!(
        "{name} goes from your personas. {} follow it and will not start until another persona is picked there or \
         it is imported again. {after}",
        library::and_list(followed)
    )
}

impl PersonasPage {
    fn toolbar(lacks: bool, cx: &Context<Self>) -> Div {
        let actions = div().flex().gap(px(8.)).when(!lacks, |d| {
            d.child(ui::icon_text_button("personas-new", Icon::Plus, "New", Tone::Plain).on_click(
                cx.listener(|this, _: &ClickEvent, window, cx| this.open_form(None, Draft::default(), window, cx)),
            ))
            .child(
                ui::icon_text_button("personas-import", Icon::Download, "Import…", Tone::Plain).on_click(
                    cx.listener(|_, _: &ClickEvent, window, cx| import::open(cx.entity().downgrade(), window, cx)),
                ),
            )
        });
        let title = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_size(px(15.)).font_weight(gpui::FontWeight::SEMIBOLD).child("Personas"))
            .child(ui::dim(HOW));
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(12.))
            .p(px(12.))
            .border_b_1()
            .border_color(colours::border())
            .child(
                ui::button("personas-back", "‹ Flows")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.back(cx))),
            )
            .child(title)
            .child(actions)
    }

    fn rows(&self, library: &Library, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .when(library.personas.is_empty() && library.broken.is_empty(), |d| d.child(ui::dim(EMPTY).p(px(10.))))
            .children(library.personas.iter().map(|persona| self.persona_row(persona, cx)))
            .children(library.broken.iter().map(|broken| self.broken_row(broken, cx)))
    }

    fn pick_row(&self, id: SharedString, pick: Pick, cx: &Context<Self>) -> Stateful<Div> {
        let selected = self.selected.as_ref() == Some(&pick);
        ui::row(id, selected).gap(px(10.)).items_start().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.selected = Some(pick.clone());
            cx.notify();
        }))
    }

    fn persona_row(&self, persona: &Persona, cx: &Context<Self>) -> Stateful<Div> {
        let face = div().w(px(22.)).flex_none().text_size(px(16.)).child(face_of(persona));
        let name = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(13.))
            .child(
                div().flex().min_w_0().font_weight(gpui::FontWeight::MEDIUM).child(ui::one_line(persona.name.clone())),
            )
            .child(div().flex_1())
            .child(div().flex_none().text_size(px(11.)).text_color(colours::faint()).child(persona.origin()));
        let text = div().flex_1().min_w_0().flex().flex_col().gap(px(2.)).child(name).children(described(persona));
        self.pick_row(SharedString::from(format!("persona-{}", persona.id)), Pick::Persona(persona.id.clone()), cx)
            .child(face)
            .child(text)
    }

    fn broken_row(&self, broken: &Unreadable, cx: &Context<Self>) -> Stateful<Div> {
        let text = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .flex()
                    .text_size(px(13.))
                    .text_color(colours::failed())
                    .child(ui::one_line(file_name(&broken.path))),
            )
            .child(error_small(ui::one_line(broken.error.clone())));
        self.pick_row(
            SharedString::from(format!("persona-broken-{}", broken.path)),
            Pick::Broken(broken.path.clone()),
            cx,
        )
        .child(div().w(px(22.)).flex_none().child(ui::icon(Icon::Alert, 15.).text_color(colours::failed())))
        .child(text)
    }
}

fn described(persona: &Persona) -> Option<Div> {
    let line = div().flex().text_size(px(11.5)).text_color(colours::muted());
    (!persona.description.is_empty()).then(|| line.child(ui::one_line(persona.description.clone())))
}

fn face_of(persona: &Persona) -> AnyElement {
    if persona.emoji.is_empty() {
        ui::icon(Icon::Agent, 15.).text_color(colours::muted()).into_any_element()
    } else {
        div().child(persona.emoji.clone()).into_any_element()
    }
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn stem(path: &str) -> String {
    let name = file_name(path);
    name.strip_suffix(".md").unwrap_or(&name).to_string()
}

impl PersonasPage {
    fn detail(&self, library: &Library, cx: &Context<Self>) -> AnyElement {
        match &self.selected {
            Some(Pick::Persona(id)) => match library.get(id) {
                Some(persona) => self.persona_detail(persona, cx).into_any_element(),
                None => ui::note("This persona is gone.").into_any_element(),
            },
            Some(Pick::Broken(path)) => match library.broken.iter().find(|broken| broken.path == *path) {
                Some(broken) => Self::broken_detail(broken, cx).into_any_element(),
                None => ui::note("This file is gone.").into_any_element(),
            },
            None => ui::note("Select a persona.").into_any_element(),
        }
    }

    fn persona_detail(&self, persona: &Persona, cx: &Context<Self>) -> Div {
        let head = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(div().text_size(px(28.)).child(face_of(persona)))
            .child(div().text_size(px(17.)).font_weight(gpui::FontWeight::SEMIBOLD).child(persona.name.clone()));
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(head)
            .when(!persona.description.is_empty(), |d| {
                d.child(div().text_color(colours::soft()).child(persona.description.clone()))
            })
            .child(facts(persona))
            .child(self.following(&persona.id))
            .child(self.persona_actions(persona, cx))
    }

    fn following(&self, id: &str) -> Div {
        let followed = followers(&self.flows, id);
        let line = if followed.is_empty() {
            "No flow follows it yet: pick it in a role's Persona row.".to_string()
        } else {
            format!("Followed in {}.", library::and_list(&followed))
        };
        ui::field("Flows", div().text_size(px(12.5)).text_color(colours::text()).child(line))
    }

    fn persona_actions(&self, persona: &Persona, cx: &Context<Self>) -> Div {
        let id = persona.id.clone();
        let edit = if self.opening.as_deref() == Some(id.as_str()) {
            ui::busy_button("persona-edit", "Opening…".to_string(), Tone::Plain).into_any_element()
        } else {
            let id = id.clone();
            ui::icon_text_button("persona-edit", Icon::Pencil, "Edit", Tone::Plain)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.edit(id.clone(), cx)))
                .into_any_element()
        };
        let name = persona.label();
        div().flex().gap(px(8.)).child(edit).child(
            ui::icon_text_button("persona-remove", Icon::Trash, "Remove…", Tone::Danger)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.remove(&id, &name, window, cx))),
        )
    }

    fn broken_detail(broken: &Unreadable, cx: &Context<Self>) -> Div {
        let id = stem(&broken.path);
        let name = file_name(&broken.path);
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colours::failed())
                    .child(name.clone()),
            )
            .child(error_text(broken.error.clone()))
            .child(ui::dim(format!(
                "This file cannot be read as a persona: fix it in an editor, or remove it. {}",
                broken.path
            )))
            .child(
                div().flex().child(
                    ui::icon_text_button("persona-broken-remove", Icon::Trash, "Remove…", Tone::Danger).on_click(
                        cx.listener(move |this, _: &ClickEvent, window, cx| this.remove(&id, &name, window, cx)),
                    ),
                ),
            )
    }

    fn body(&self, cx: &Context<Self>) -> AnyElement {
        let lacks = Store::get(cx).read(cx).releases().lacks(Feature::FlowPersonas);
        if let Some(lacks) = lacks {
            let needs = cli::newer(Feature::FlowPersonas.since(), lacks, Os::CURRENT);
            return div().p(px(20.)).child(ui::note(format!("Personas need {needs}."))).into_any_element();
        }
        let library = match &self.library {
            None => return div().p(px(20.)).child(ui::loading("Reading your personas…")).into_any_element(),
            Some(Err(e)) => return div().p(px(20.)).child(error_text(e.clone())).into_any_element(),
            Some(Ok(library)) => library,
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .child(
                div()
                    .id("personas-list")
                    .w(px(LIST_WIDTH))
                    .flex_none()
                    .h_full()
                    .overflow_y_scroll()
                    .p(px(8.))
                    .border_r_1()
                    .border_color(colours::border())
                    .child(self.rows(library, cx)),
            )
            .child(
                div()
                    .id("personas-detail")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .p(px(20.))
                    .child(self.detail(library, cx)),
            )
            .into_any_element()
    }
}

fn facts(persona: &Persona) -> Div {
    let source = persona.source.as_ref().map_or_else(|| "yours".to_string(), |source| format!("from {source}"));
    let line = |label: &'static str, text: String| {
        ui::field(label, div().text_size(px(12.5)).text_color(colours::text()).child(text))
    };
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(line("Id", format!("{} · {}", persona.id, kib(persona.bytes))))
        .child(line("Source", source))
        .children(persona.path.clone().map(|path| line("File", ui::home_relative(&path))))
}

impl Render for PersonasPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(form) = &self.form {
            return div().size_full().child(form.clone());
        }
        let lacks = Store::get(cx).read(cx).releases().lacks(Feature::FlowPersonas).is_some();
        div().size_full().flex().flex_col().child(Self::toolbar(lacks, cx)).child(self.body(cx))
    }
}
