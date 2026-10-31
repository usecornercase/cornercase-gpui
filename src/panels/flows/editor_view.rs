use gpui::{
    AnyElement, App, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::draft::Place;
use super::editor::{FlowEditor, Mode, RoleFields, StageFields, Text, Verdict};
use super::flow_page::block;
use super::widgets::{alert, error_text, icon_action, panel_box};
use crate::theme::colours;
use crate::ui::{self, Icon, TextField, Tone, boxed};

const KEPT: &str = "It has line breaks, which this one-line field would join: edit it in the flow's file.";

fn labelled(label: &str, field: &gpui::Entity<TextField>, window: &Window, cx: &App) -> Div {
    ui::field(label.to_string(), boxed(field, window, cx)).flex_1().min_w_0()
}

fn text_control(label: &str, text: &Text, window: &Window, cx: &App) -> Div {
    match text {
        Text::Field(field) => labelled(label, field, window, cx),
        Text::Kept(text) => ui::field(
            label.to_string(),
            panel_box()
                .p(px(10.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(div().text_size(px(12.)).text_color(colours::soft()).child(text.clone()))
                .child(ui::dim(KEPT)),
        ),
    }
}

fn line() -> Div {
    div().flex().gap(px(10.)).items_start()
}

impl FlowEditor {
    fn header(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(16.))
            .border_b_1()
            .border_color(colours::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui::title(self.title.clone()).text_size(px(15.)).flex_1())
                    .child(self.buttons(cx)),
            )
            .child(self.place_line(cx))
            .children(self.note.clone().map(ui::dim))
            .children((self.mode == Mode::Discarding).then(|| Self::discard_ask(cx)))
    }

    fn buttons(&self, cx: &Context<Self>) -> Div {
        let save = if self.mode == Mode::Saving {
            ui::busy_button("editor-save", "Saving…", Tone::Primary)
        } else {
            ui::icon_text_button("editor-save", Icon::Download, "Save", Tone::Primary)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx)))
        };
        div()
            .flex()
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

    fn place_line(&self, cx: &Context<Self>) -> AnyElement {
        if let Place::File(path) = &self.place {
            return ui::dim(format!("Saves {path}")).into_any_element();
        }
        if !self.new || self.project.is_none() {
            return ui::dim("Saves into your flows").into_any_element();
        }
        let choice = |id: &'static str, label: &'static str, place: Place| {
            let selected = self.place == place;
            ui::segment(id, label, selected)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_place(place.clone(), cx)))
                .into_any_element()
        };
        ui::field(
            "Into",
            ui::segments(vec![
                choice("editor-yours", "Yours", Place::Yours),
                choice("editor-project", "This project's", Place::Project),
            ]),
        )
        .into_any_element()
    }

    fn verdict_view(&self) -> Div {
        let state = div().flex().flex_col().gap(px(6.));
        let state = match &self.verdict {
            Verdict::Waiting | Verdict::Checking => state.child(ui::busy("verdict-checking", "Checking…")),
            Verdict::Invalid(e) => state.child(alert(e.clone(), colours::failed())),
            Verdict::Checked(saved) if saved.problems.is_empty() => state
                .child(ui::dim("No problems: it can be saved."))
                .children(saved.check.clone().map(|c| alert(c, colours::waiting()))),
            Verdict::Checked(saved) => {
                state.children(saved.problems.iter().map(|p| alert(p.clone(), colours::failed())))
            }
        };
        state.children(self.error.clone().map(error_text))
    }

    fn flow_fields(&self, window: &Window, cx: &Context<Self>) -> Div {
        let end = |id: &'static str, label: &'static str, commits: bool| {
            ui::segment(id, label, self.commits == commits)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_commits(commits, cx)))
                .into_any_element()
        };
        block(Icon::Flows, "Flow")
            .child(labelled("Name", &self.name, window, cx))
            .child(text_control("Description", &self.description, window, cx))
            .child(text_control("Notes", &self.notes, window, cx))
            .child(ui::field(
                "Then",
                ui::segments(vec![
                    end("editor-commits", "Leave the commits", true),
                    end("editor-pr", "Open a pull request", false),
                ]),
            ))
    }

    fn agent_line(role: &RoleFields, window: &Window, cx: &App) -> Div {
        line()
            .child(labelled("Name", &role.name, window, cx))
            .child(labelled("Agent", &role.agent, window, cx))
            .child(labelled("Model", &role.model, window, cx))
            .child(labelled("Effort", &role.effort, window, cx))
    }

    fn orchestrator_fields(&self, window: &Window, cx: &Context<Self>) -> Div {
        let lead = &self.orchestrator;
        block(Icon::Waypoints, "Orchestrator").child(Self::agent_line(lead, window, cx)).child(text_control(
            "Prompt",
            &lead.prompt,
            window,
            cx,
        ))
    }

    fn role_card(index: usize, role: &RoleFields, window: &Window, cx: &Context<Self>) -> Div {
        let toggle = |id: &'static str, checked: bool, label: &'static str, own: bool| {
            ui::checkbox((id, index), checked, label)
                .text_size(px(12.))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_role(index, own, cx)))
        };
        panel_box()
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(Self::agent_line(role, window, cx))
            .child(text_control("Prompt", &role.prompt, window, cx))
            .child(
                line()
                    .items_center()
                    .gap(px(16.))
                    .child(toggle("role-edits", role.edits, "Edits the files", false))
                    .child(toggle("role-own", role.own_worktree, "A worktree of its own", true))
                    .child(div().flex_1())
                    .child(
                        icon_action(("role-remove", index), Icon::Trash, "Remove this role", true)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.remove_role(index, cx))),
                    ),
            )
    }

    fn role_fields(&self, window: &Window, cx: &Context<Self>) -> Div {
        block(Icon::Accounts, "Roles")
            .children(self.roles.iter().enumerate().map(|(i, role)| Self::role_card(i, role, window, cx)))
            .child(adder(
                "editor-add-role",
                "Add role",
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.add_role(window, cx);
                }),
            ))
    }

    fn stage_row(index: usize, stage: &StageFields, window: &Window, cx: &Context<Self>) -> Div {
        line()
            .child(
                div()
                    .w(px(22.))
                    .pt(px(26.))
                    .flex_none()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colours::muted())
                    .child((index + 1).to_string()),
            )
            .child(labelled("Roles, side by side", &stage.roles, window, cx).flex_grow())
            .child(labelled("Back to stage", &stage.back_to, window, cx).max_w(px(110.)))
            .child(labelled("When", &stage.when, window, cx).max_w(px(130.)))
            .child(labelled("At most", &stage.at_most, window, cx).max_w(px(80.)))
            .child(
                div().pt(px(20.)).child(
                    icon_action(("stage-remove", index), Icon::Trash, "Remove this stage", true)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.remove_stage(index, cx))),
                ),
            )
    }

    fn stage_fields(&self, window: &Window, cx: &Context<Self>) -> Div {
        block(Icon::Repeat, "Stages")
            .child(ui::dim(
                "Stages run top to bottom; the roles of one stage work side by side. A loop sends the work back \
                 to an earlier stage, at most so many times.",
            ))
            .children(self.stages.iter().enumerate().map(|(i, stage)| Self::stage_row(i, stage, window, cx)))
            .child(adder(
                "editor-add-stage",
                "Add stage",
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.add_stage(window, cx);
                }),
            ))
    }

    fn body(&self, window: &Window, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(self.verdict_view())
            .child(self.flow_fields(window, cx))
            .child(self.orchestrator_fields(window, cx))
            .child(self.role_fields(window, cx))
            .child(self.stage_fields(window, cx))
            .child(ui::dim("A role's arguments and variables are kept as they are; change them in the flow's file."))
    }
}

fn adder(
    id: &'static str,
    label: &'static str,
    click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div().flex().child(ui::icon_text_button(id, Icon::Plus, label, Tone::Plain).on_click(click))
}

impl Render for FlowEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().flex_col().child(self.header(cx)).child(
            div()
                .id(SharedString::from("flow-editor-body"))
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(20.))
                .child(self.body(window, cx).pb(px(12.))),
        )
    }
}
