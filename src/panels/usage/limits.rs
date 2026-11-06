use gpui::{
    Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Rgba, Styled as _, div,
    prelude::FluentBuilder as _, px,
};

use super::data::{self, Limit, Plan, Severity};
use super::parts::{Part, Row};
use super::widgets::{card, error_line};
use crate::model::machines::LinkState;
use crate::theme::colours;
use crate::ui;

fn severity_colour(severity: Severity) -> Rgba {
    match severity {
        Severity::Normal => colours::done(),
        Severity::Warning => colours::waiting(),
        Severity::Critical => colours::failed(),
    }
}

pub(super) fn part(i: usize, part: &Part, fresh: bool, now: i64) -> impl IntoElement + use<> {
    let fresh = fresh && part.state == LinkState::Linked;
    div()
        .id(("usage-part", i))
        .flex()
        .flex_col()
        .gap(px(14.))
        .children(part.name.as_deref().map(|name| part_heading(name, &part.status, why_line(part, fresh))))
        .when(part.rows.is_empty() && part.why.is_none(), |d| d.child(no_rows(part)))
        .children(part.rows.iter().enumerate().map(|(i, row)| plan_card(i, row, fresh, now)))
}

fn part_heading(name: &str, status: &str, why: Option<Div>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(name.to_string()))
                .child(ui::dim(status.to_string())),
        )
        .children(why)
}

fn why_line(part: &Part, fresh: bool) -> Option<Div> {
    let why = part.why.as_ref()?;
    let name = part.name.as_deref().unwrap_or_default();
    Some(match part.state {
        LinkState::Connecting => ui::busy("usage-why", why.clone()),
        LinkState::Linked if part.unread && fresh => ui::busy("usage-why", format!("Asking {name}…")),
        _ => error_line(why.clone()),
    })
}

fn no_rows(part: &Part) -> Div {
    if part.joined > 0 {
        return ui::dim("Its accounts are listed above.");
    }
    ui::note("No agent with plan limits is installed.")
}

fn plan_card(i: usize, row: &Row, fresh: bool, now: i64) -> impl IntoElement + use<> {
    let plan = &row.plan;
    let status = if fresh { "loading…".to_string() } else { row.status(now) };
    card()
        .id(("usage-plan", i))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(plan.heading()))
                        .child(div().flex_1())
                        .child(ui::dim(status)),
                )
                .when_some(row.on_line(), |d, on| d.child(ui::dim(on))),
        )
        .when_some(plan.error.as_ref(), |d, e| d.child(error_line(format!("usage unavailable: {e}"))))
        .when(!plan.limits.is_empty(), |d| d.child(limits(plan, now)))
        .when_some(plan.notice.as_ref(), |d, n| d.child(div().text_color(colours::muted()).child(n.clone())))
        .when(plan.limited == Some(false) && plan.notice.is_none(), |d| {
            d.child(div().text_color(colours::muted()).child("no plan limits"))
        })
        .when_some(plan.extra.as_ref(), |d, e| d.child(ui::dim(e.clone())))
        .when(plan.unasked() && !fresh, |d| d.child(ui::dim("not asked yet")))
}

fn limits(plan: &Plan, now: i64) -> Div {
    div().flex().flex_col().gap(px(14.)).children(plan.limits.iter().map(|l| limit_row(l, now)))
}

fn limit_row(limit: &Limit, now: i64) -> impl IntoElement + use<> {
    let colour = severity_colour(limit.severity());
    let resets =
        limit.resets_at.as_deref().and_then(data::parse_utc).map(|at| data::until(at - now)).unwrap_or_default();
    div()
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .child(limit.label.clone())
                .child(div().flex_1())
                .child(ui::dim(resets))
                .child(
                    div()
                        .w(px(44.))
                        .flex()
                        .justify_end()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colour)
                        .child(format!("{:.0}%", limit.percent)),
                ),
        )
        .child(ui::bar(limit.fraction(), colour, 6.))
}
