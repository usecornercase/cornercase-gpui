use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::model::{Broken, FlowInfo, Listing, Run};
use super::widgets::{agents_label, error_small, file_name, second_line, tag};
use super::{FlowsPanel, Selected};
use crate::model::Session;
use crate::store::Store;
use crate::theme::colours;
use crate::ui;

impl FlowsPanel {
    pub(super) fn sidebar(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        div()
            .w(px(330.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(colours::border())
            .child(toolbar(cx))
            .child(div().id("flows-list").flex_1().overflow_y_scroll().px(px(8.)).pb(px(12.)).child(self.list(cx)))
    }

    fn list(&self, cx: &Context<Self>) -> Div {
        let session = Store::get(cx).read(cx).session.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .child(ui::section("Runs"))
            .when(self.runs.is_empty(), |d| d.child(ui::dim("No flow is running.").px(px(8.)).pb(px(4.))))
            .when_some(self.runs_error.clone(), |d, err| d.child(error_small(err).px(px(8.))))
            .children(self.runs.iter().map(|run| self.run_row(run, session.as_ref(), cx)))
            .child(ui::section("Flows"))
            .children(self.listing_state())
            .children(self.listing.iter().flat_map(|listing| self.listing_rows(listing, cx)))
    }

    fn pick(&self, id: impl Into<ElementId>, selection: Selected, cx: &Context<Self>) -> Stateful<Div> {
        let selected = self.selected.as_ref() == Some(&selection);
        ui::row(id, selected)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(selection.clone(), cx)))
    }

    fn run_row(&self, run: &Run, session: Option<&Session>, cx: &Context<Self>) -> Stateful<Div> {
        let id = run.id;
        self.pick(SharedString::from(format!("run-{id}")), Selected::Run(id), cx)
            .items_start()
            .child(ui::badge(if run.closed() { None } else { run.loudest() }).pt(px(1.)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap(px(6.))
                            .child(div().truncate().text_size(px(13.)).child(run.flow.name.clone()))
                            .child(ui::dim(run_state(run)).flex_none()),
                    )
                    .child(ui::dim(run_place(run, session)).truncate())
                    .child(ui::dim(run.headline().to_string()).truncate()),
            )
    }

    fn listing_state(&self) -> Option<Div> {
        match (&self.listing, &self.list_error) {
            (_, Some(err)) => Some(error_small(err.clone()).px(px(8.))),
            (None, None) => Some(ui::dim("Reading the flows…").px(px(8.))),
            _ => None,
        }
    }

    fn listing_rows(&self, listing: &Listing, cx: &Context<Self>) -> Vec<AnyElement> {
        let mut rows: Vec<AnyElement> =
            listing.flows.iter().enumerate().map(|(i, flow)| self.flow_row(i, flow, cx).into_any_element()).collect();
        if !listing.hidden.is_empty() {
            rows.push(ui::section("Deleted built-ins").into_any_element());
            rows.extend(listing.hidden.iter().enumerate().map(|(i, flow)| self.hidden_row(i, flow, cx)));
        }
        if !listing.broken.is_empty() {
            rows.push(ui::section("Cannot be read").into_any_element());
            rows.extend(listing.broken.iter().enumerate().map(|(i, broken)| self.broken_row(i, broken, cx)));
        }
        rows
    }

    fn flow_row(&self, i: usize, flow: &FlowInfo, cx: &Context<Self>) -> Stateful<Div> {
        self.pick(("flow", i), Selected::Flow(flow.key()), cx).child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(div().truncate().text_size(px(13.)).child(flow.name.clone()))
                        .children(flow.tags().into_iter().map(tag))
                        .child(div().flex_1())
                        .child(ui::dim(agents_label(flow)).flex_none()),
                )
                .child(second_line(flow)),
        )
    }

    fn hidden_row(&self, i: usize, flow: &FlowInfo, cx: &Context<Self>) -> AnyElement {
        self.pick(("hidden", i), Selected::Hidden(flow.name.clone()), cx)
            .child(div().flex_1().text_size(px(13.)).text_color(colours::muted()).child(flow.name.clone()))
            .into_any_element()
    }

    fn broken_row(&self, i: usize, broken: &Broken, cx: &Context<Self>) -> AnyElement {
        self.pick(("broken", i), Selected::Broken(broken.path.clone()), cx)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().text_size(px(13.)).child(file_name(&broken.path)))
                    .child(error_small(broken.error.clone()).truncate()),
            )
            .into_any_element()
    }
}

fn toolbar(cx: &Context<FlowsPanel>) -> impl IntoElement + use<> {
    div()
        .flex()
        .gap(px(6.))
        .p(px(10.))
        .border_b_1()
        .border_color(colours::border())
        .child(ui::primary_button("flows-start", "Start a flow…").on_click(cx.listener(
            |this, _: &ClickEvent, window, cx| {
                let name = this.selected_flow().map(FlowInfo::key);
                this.open_start(name, window, cx);
            },
        )))
        .child(
            ui::button("flows-import", "Import…")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_import(window, cx))),
        )
}

fn run_state(run: &Run) -> String {
    if run.closed() {
        "closed".to_string()
    } else if run.ended {
        "ended".to_string()
    } else {
        run.summary()
    }
}

fn run_place(run: &Run, session: Option<&Session>) -> String {
    session.and_then(|s| s.workspace(run.workspace)).map_or_else(
        || format!("workspace {}", run.workspace),
        |(p, w)| format!("{} › {}", p.name, w.branch.clone().unwrap_or_else(|| w.name.clone())),
    )
}
