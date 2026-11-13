use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use super::draft::saves_line;
use super::editor::{FlowEditor, Layout, Mode, PLAN_PAD, Pill, View, layout};
use super::widgets::{alert, glyph, own_axis, pill, pill_with};
use super::wires;
use crate::theme::colours;
use crate::ui::{self, Icon, TextField, Tone, boxed};

pub(super) fn labelled(label: &str, field: &Entity<TextField>, window: &Window, cx: &App) -> Div {
    ui::field(label.to_string(), boxed(field, window, cx)).flex_1().min_w_0()
}

pub(super) fn text_control(label: &str, field: &Entity<TextField>, window: &Window, cx: &App) -> Div {
    ui::field(label.to_string(), boxed(field, window, cx))
}

pub(super) fn line() -> Div {
    div().flex().gap(px(10.)).items_start()
}

pub(super) fn adder(
    id: impl Into<ElementId>,
    glyph: Icon,
    label: &'static str,
    dashed: bool,
    click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let button = ui::icon_text_button(id, glyph, label, Tone::Plain).h(px(26.)).text_size(px(12.)).on_click(click);
    div().flex().child(button.when(dashed, gpui::Styled::border_dashed))
}

pub(super) fn choice(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &Context<FlowEditor>,
    act: impl Fn(&mut FlowEditor, &mut Window, &mut Context<FlowEditor>) + 'static,
) -> AnyElement {
    ui::segment(id, label, selected)
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| act(this, window, cx)))
        .into_any_element()
}

impl FlowEditor {
    fn view_choice(&self, glyph: Icon, label: &'static str, view: View, cx: &Context<Self>) -> AnyElement {
        ui::icon_segment(label, glyph, label, self.view == view)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_view(view, cx)))
            .into_any_element()
    }

    fn pill(&self) -> Div {
        let state = self.check.pill();
        let words = state.words();
        match state {
            Pill::Checking => pill_with(ui::spinner("editor-checking", 10., colours::muted()), words, colours::muted()),
            Pill::Unchecked => pill(words, colours::muted(), false),
            Pill::Ready => pill(words, colours::done(), true),
            Pill::Problems(_) => pill(words, colours::failed(), true),
        }
    }

    fn title_column(&self) -> Div {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .flex()
                    .text_size(px(15.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(ui::one_line(self.title.clone())),
            )
            .child(div().flex().child(ui::dim(ui::one_line(saves_line(&self.place, self.note.as_deref())))))
    }

    fn header(&self, layout: Layout, cx: &Context<Self>) -> Div {
        let row = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(glyph(&self.plan, None, 36.))
            .child(self.title_column())
            .when(layout.plan.is_none(), |row| {
                row.child(ui::segments(vec![
                    self.view_choice(Icon::Todo, "Form", View::Form, cx),
                    self.view_choice(Icon::Flows, "Plan", View::Plan, cx),
                ]))
            })
            .child(self.pill())
            .child(self.buttons(cx));
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(18.))
            .py(px(14.))
            .border_b_1()
            .border_color(colours::border())
            .child(row)
            .children((self.mode == Mode::Discarding).then(|| Self::discard_ask(cx)))
    }

    fn buttons(&self, cx: &Context<Self>) -> Div {
        let save = if self.mode == Mode::Saving {
            ui::busy_button("editor-save", "Saving…", Tone::Primary)
        } else if self.check.rests() {
            ui::resting_button("editor-save", "Save", Tone::Primary)
        } else {
            ui::icon_text_button("editor-save", Icon::Download, "Save", Tone::Primary)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx)))
        };
        div()
            .flex()
            .flex_none()
            .gap(px(8.))
            .child(
                ui::button("editor-cancel", "Cancel")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx))),
            )
            .child(save)
    }

    fn discard_ask(cx: &Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().text_size(px(12.)).text_color(colours::waiting()).child("Discard your changes?"))
            .child(
                ui::button("editor-keep", "Keep editing")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.keep_editing(cx))),
            )
            .child(
                ui::danger_button("editor-discard", "Discard")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx))),
            )
    }

    fn strip(&self) -> Option<impl IntoElement + use<>> {
        let failed = self.check.problems().into_iter().chain(self.error.clone());
        let red = failed.map(|text| alert(text, colours::failed()));
        let orange = self.opened.iter().map(String::as_str).chain(self.check.risk());
        let orange = orange.map(|text| alert(text.to_string(), colours::waiting()));
        let lines: Vec<Div> = red.chain(orange).collect();
        (!lines.is_empty()).then(|| {
            div()
                .id("flow-editor-strip")
                .flex_none()
                .max_h(px(160.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(6.))
                .px(px(18.))
                .py(px(10.))
                .border_b_1()
                .border_color(colours::border())
                .children(lines)
        })
    }

    fn form_column(&self, layout: Layout, window: &Window, cx: &Context<Self>) -> impl IntoElement + use<> {
        let form = div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .child(self.about(layout, window, cx))
            .child(self.team(window, cx))
            .child(self.end(cx));
        div()
            .id("flow-editor-form")
            .track_scroll(&self.inputs.scroller)
            .line_height(relative(1.35))
            .h_full()
            .overflow_y_scroll()
            .px(px(18.))
            .pt(px(18.))
            .pb(px(22.))
            .child(form)
    }

    fn plan_box(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        own_axis(div().id("flow-editor-plan"))
            .h_full()
            .overflow_y_scroll()
            .px(px(PLAN_PAD))
            .py(px(18.))
            .child(self.plan_column(cx))
    }

    fn body(&self, layout: Layout, window: &Window, cx: &Context<Self>) -> Div {
        let body = div().flex_1().min_h_0();
        match layout.plan {
            Some(plan) => {
                body.flex().child(div().flex_1().min_w_0().child(self.form_column(layout, window, cx))).child(
                    div().w(px(plan)).flex_none().border_l_1().border_color(colours::border()).child(self.plan_box(cx)),
                )
            }
            None if self.view == View::Plan => body.child(self.plan_box(cx)),
            None => body.child(self.form_column(layout, window, cx)),
        }
    }
}

impl Render for FlowEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = layout(self.width.get(), self.narrowest());
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .child(wires::measure(self.width.clone()))
            .child(self.header(layout, cx))
            .children(self.strip())
            .child(self.body(layout, window, cx))
    }
}
