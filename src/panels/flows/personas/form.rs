use gpui::{
    ClickEvent, Context, Div, Entity, Focusable as _, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    ScrollHandle, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};

use super::PersonasPage;
use crate::cli;
use crate::dialogs::{self, Question};
use crate::panels::flows::actions::{run_stdin, toast};
use crate::panels::flows::dialog::{Submit, boxed, filled_field, prompt_field};
use crate::panels::flows::library::{Draft, parse_saved};
use crate::panels::flows::widgets::error_text;
use crate::store::ToastKind;
use crate::theme::colours;
use crate::ui::{self, Icon, TextField, Tone};

const KEPT: &str = "Saving keeps its id, so the flows that follow it keep it, and its other keys (such as `color`); \
                    it becomes yours and loses its source.";
const NEW: &str = "It is yours: no source. Its id comes from the name.";

pub(super) struct PersonaForm {
    page: WeakEntity<PersonasPage>,
    editing: Option<String>,
    original: Draft,
    name: Entity<TextField>,
    description: Entity<TextField>,
    emoji: Entity<TextField>,
    body: Entity<TextField>,
    scroller: ScrollHandle,
    busy: bool,
    error: Option<String>,
}

impl PersonaForm {
    pub(super) fn new(
        page: WeakEntity<PersonasPage>,
        (editing, original): (Option<String>, Draft),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let scroller = ScrollHandle::new();
        let name = filled_field("Careful Tester", &original.name, &scroller, cx, window);
        let description = filled_field("One line on what it is for", &original.description, &scroller, cx, window);
        let emoji = filled_field("🧪", &original.emoji, &scroller, cx, window);
        let body = prompt_field("What the agent should do and how, in Markdown", &original.body, &scroller, cx, window);
        window.focus(&name.read(cx).focus_handle(cx));
        Self { page, editing, original, name, description, emoji, body, scroller, busy: false, error: None }
    }

    fn draft(&self, cx: &gpui::App) -> Draft {
        Draft {
            name: self.name.read(cx).text(),
            description: self.description.read(cx).text(),
            emoji: self.emoji.read(cx).text(),
            body: self.body.read(cx).text(),
        }
    }

    pub(super) fn dirty(&self, cx: &gpui::App) -> bool {
        self.draft(cx) != self.original
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty(cx) {
            self.close(None, cx);
            return;
        }
        let page = self.page.clone();
        let question = Question::new(
            "Discard your changes?",
            "What you wrote in this persona is not saved.",
            "Discard",
            Vec::new(),
        )
        .on_yes(move |cx| {
            let _ = page.update(cx, |page, cx| page.form_closed(None, cx));
        });
        dialogs::confirm(question, window, cx);
    }

    fn close(&self, saved: Option<String>, cx: &mut Context<Self>) {
        let _ = self.page.update(cx, |page, cx| page.form_closed(saved, cx));
    }

    fn saved(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.busy = false;
        match result.and_then(|out| parse_saved(&out)) {
            Ok(saved) => {
                toast(format!("Saved {}", saved.persona.label()), ToastKind::Done, cx);
                self.close(Some(saved.persona.id), cx);
            }
            Err(e) => self.error = Some(cli::reason(&e)),
        }
        cx.notify();
    }

    fn head(&self) -> Div {
        let title = match &self.editing {
            Some(_) => format!("Edit {}", self.original.name),
            None => "New persona".to_string(),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().text_size(px(16.)).font_weight(gpui::FontWeight::SEMIBOLD).child(title))
            .child(ui::dim(if self.editing.is_some() { KEPT } else { NEW }))
    }

    fn fields(&self, window: &Window, cx: &Context<Self>) -> Div {
        let top = div()
            .flex()
            .gap(px(10.))
            .child(ui::field("Emoji", boxed(&self.emoji, window, cx)).w(px(90.)).flex_none())
            .child(ui::field("Name", boxed(&self.name, window, cx)).flex_1().min_w_0());
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(top)
            .child(ui::field("Description", boxed(&self.description, window, cx)))
            .child(ui::field("Instructions", boxed(&self.body, window, cx)))
            .children(self.error.clone().map(error_text))
    }

    fn actions(&self, cx: &Context<Self>) -> Div {
        let save = if self.busy {
            ui::busy_button("persona-save", "Saving…".to_string(), Tone::Primary).into_any_element()
        } else {
            ui::icon_text_button("persona-save", Icon::Check, "Save", Tone::Primary)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx)))
                .into_any_element()
        };
        div()
            .flex()
            .justify_end()
            .gap(px(8.))
            .child(
                ui::button("persona-cancel", "Cancel")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.cancel(window, cx))),
            )
            .child(save)
    }
}

impl Submit for PersonaForm {
    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let draft = self.draft(cx);
        if let Some(problem) = draft.problem() {
            self.error = Some(problem.into());
            cx.notify();
            return;
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        run_stdin(cx, draft.args(self.editing.as_deref()), draft.body, Self::saved);
    }
}

impl Render for PersonaForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let form = div()
            .w_full()
            .max_w(px(720.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(self.head())
            .child(self.fields(window, cx));
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("persona-form")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroller)
                    .p(px(20.))
                    .child(form),
            )
            .child(div().p(px(12.)).border_t_1().border_color(colours::border()).child(self.actions(cx)))
    }
}
