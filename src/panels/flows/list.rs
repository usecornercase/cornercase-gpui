use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Rgba, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px,
};

use super::cards::badge;
use super::model::{Broken, FlowInfo, Listing, Run};
use super::widgets::{error_small, file_name, flow_summary, glyph, name_line};
use super::{FlowsPanel, Selected};
use crate::model::Session;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, Icon, Tone};

const GLYPH: f32 = 36.;

impl FlowsPanel {
    pub(super) fn sidebar(&self, width: f32, cx: &Context<Self>) -> impl IntoElement + use<> {
        div()
            .w(px(width))
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
        let flows = self.listing.as_ref().map_or(0, |l| l.flows.len());
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(group("Runs", self.runs.len()))
            .when(self.runs.is_empty(), |d| d.child(ui::dim("No flow is running.").px(px(10.)).pb(px(4.))))
            .when_some(self.runs_error.clone(), |d, err| d.child(error_small(err).px(px(10.))))
            .children(self.runs.iter().map(|run| self.run_row(run, session.as_ref(), cx)))
            .child(group("Flows", flows))
            .children(self.listing_state())
            .children(self.listing.iter().flat_map(|listing| self.listing_rows(listing, cx)))
    }

    fn pick(&self, id: impl Into<ElementId>, selection: Selected, cx: &Context<Self>) -> Stateful<Div> {
        let selected = self.selected.as_ref() == Some(&selection);
        ui::row(id, selected)
            .gap(px(10.))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(selection.clone(), cx)))
    }

    fn run_row(&self, run: &Run, session: Option<&Session>, cx: &Context<Self>) -> Stateful<Div> {
        let id = run.id;
        let (state, colour) = run_state(run);
        self.pick(SharedString::from(format!("run-{id}")), Selected::Run(id), cx)
            .items_start()
            .child(badge(glyph(&run.flow, Some(run), GLYPH), run.loudest()).when(run.closed(), |d| d.opacity(0.5)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        name_line(run.flow.name.clone())
                            .child(div().flex_1())
                            .child(div().flex_none().text_size(px(11.)).text_color(colour).child(state)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(colours::soft())
                            .child(ui::one_line(run.headline().to_string())),
                    )
                    .child(ui::dim(ui::one_line(format!("{} · {}", run_place(run, session), run.summary())))),
            )
    }

    fn listing_state(&self) -> Option<Div> {
        match (&self.listing, &self.list_error) {
            (_, Some(err)) => Some(error_small(err.clone()).px(px(10.))),
            (None, None) => Some(ui::busy("flows-loading", "Reading the flows…").px(px(10.))),
            _ => None,
        }
    }

    fn listing_rows(&self, listing: &Listing, cx: &Context<Self>) -> Vec<AnyElement> {
        let mut rows: Vec<AnyElement> =
            listing.flows.iter().enumerate().map(|(i, flow)| self.flow_row(i, flow, cx).into_any_element()).collect();
        if !listing.hidden.is_empty() {
            rows.push(group("Deleted built-ins", listing.hidden.len()).into_any_element());
            rows.extend(listing.hidden.iter().enumerate().map(|(i, flow)| self.hidden_row(i, flow, cx)));
        }
        if !listing.broken.is_empty() {
            rows.push(group("Cannot be read", listing.broken.len()).into_any_element());
            rows.extend(listing.broken.iter().enumerate().map(|(i, broken)| self.broken_row(i, broken, cx)));
        }
        rows
    }

    fn flow_row(&self, i: usize, flow: &FlowInfo, cx: &Context<Self>) -> Stateful<Div> {
        self.pick(("flow", i), Selected::Flow(flow.key()), cx)
            .child(glyph(&flow.flow, None, GLYPH))
            .child(flow_summary(flow))
    }

    fn hidden_row(&self, i: usize, flow: &FlowInfo, cx: &Context<Self>) -> AnyElement {
        self.pick(("hidden", i), Selected::Hidden(flow.name.clone()), cx)
            .child(glyph(&flow.flow, None, GLYPH).opacity(0.4))
            .child(div().flex_1().text_size(px(13.)).text_color(colours::muted()).child(flow.name.clone()))
            .into_any_element()
    }

    fn broken_row(&self, i: usize, broken: &Broken, cx: &Context<Self>) -> AnyElement {
        self.pick(("broken", i), Selected::Broken(broken.path.clone()), cx)
            .child(
                div()
                    .size(px(GLYPH))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(colours::border())
                    .child(ui::icon(Icon::File, 16.).text_color(colours::failed())),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().text_size(px(13.)).child(ui::one_line(file_name(&broken.path))))
                    .child(error_small(ui::one_line(broken.error.clone()))),
            )
            .into_any_element()
    }
}

fn group(title: &str, count: usize) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(10.))
        .pt(px(16.))
        .pb(px(6.))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colours::muted())
        .child(title.to_uppercase())
        .child(div().text_color(colours::faint()).font_weight(FontWeight::NORMAL).child(count.to_string()))
}

fn toolbar(cx: &Context<FlowsPanel>) -> impl IntoElement + use<> {
    div()
        .flex()
        .gap(px(8.))
        .p(px(12.))
        .border_b_1()
        .border_color(colours::border())
        .child(ui::icon_text_button("flows-start", Icon::Play, "Start a flow…", Tone::Primary).on_click(cx.listener(
            |this, _: &ClickEvent, window, cx| {
                let name = this.selected_flow().map(FlowInfo::key);
                this.open_start(name, window, cx);
            },
        )))
        .child(
            ui::icon_text_button("flows-new", Icon::Plus, "New", Tone::Plain)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_editor(None, window, cx))),
        )
        .child(
            ui::icon_text_button("flows-import", Icon::Download, "Import…", Tone::Plain)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_import(window, cx))),
        )
}

fn run_state(run: &Run) -> (&'static str, Rgba) {
    if run.closed() {
        ("closed", colours::faint())
    } else if run.ended {
        ("ended", colours::accent())
    } else {
        ("running", colours::working())
    }
}

fn run_place(run: &Run, session: Option<&Session>) -> String {
    session.and_then(|s| s.workspace(run.workspace)).map_or_else(
        || format!("workspace {}", run.workspace),
        |(p, w)| format!("{} › {}", p.name, w.branch.clone().unwrap_or_else(|| w.name.clone())),
    )
}
