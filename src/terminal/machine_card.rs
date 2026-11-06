use std::time::Duration;

use gpui::{
    AnyElement, AsyncApp, Context, Div, IntoElement as _, ParentElement as _, Styled as _, WeakEntity, Window, div, px,
};

use super::no_tab::{heading, lead_text};
use crate::app::Root;
use crate::model::{MachinePlace, MachineSeen, Tab, ThereId};
use crate::platform::paths;
use crate::store::ToastKind;
use crate::theme::{colours, radius};
use crate::ui;

const SEEN_EVERY: Duration = Duration::from_secs(1);

#[derive(Debug, PartialEq, Eq)]
pub(super) struct CardText {
    pub(super) title: String,
    pub(super) line: String,
    pub(super) folder: String,
}

fn agent_of(tab: &Tab<ThereId>) -> Option<(&str, Option<&str>)> {
    let agents = || tab.panes.iter().filter(|pane| pane.agent.is_some());
    let pane = agents().find(|pane| pane.active).or_else(|| agents().next())?;
    Some((pane.agent.as_deref()?, pane.status.as_deref()))
}

fn doing(status: Option<&str>) -> String {
    match status {
        Some("waiting") => "is waiting for you".to_string(),
        Some("failed") => "failed".to_string(),
        Some(status) => format!("is {status}"),
        None => "is idle".to_string(),
    }
}

pub(super) fn card_text(place: &MachinePlace, seen: &MachineSeen, home: Option<&str>) -> CardText {
    let tab = seen.workspace(place.workspace).and_then(|ws| ws.tab(place.tab));
    let agent = tab.and_then(agent_of);
    let folder = home.map_or_else(|| place.path().to_string(), |home| paths::home_relative(place.path(), home));
    if seen.linked() {
        let line =
            agent.map_or_else(|| "no agent there".to_string(), |(agent, status)| format!("{agent} {}", doing(status)));
        return CardText { title: format!("runs on {}", seen.name), line, folder };
    }
    let line = agent.map_or_else(
        || "its shells keep running there".to_string(),
        |(agent, _)| format!("{agent} keeps working there"),
    );
    CardText { title: seen.link_note(), line, folder }
}

async fn keep_counting(this: WeakEntity<Root>, cx: &mut AsyncApp) {
    loop {
        cx.background_executor().timer(SEEN_EVERY).await;
        if this.update(cx, Root::count_seen_up).is_err() {
            return;
        }
    }
}

fn machine_card(text: &CardText, seen: &MachineSeen) -> Div {
    let card = ui::glass::card(radius::SHEET)
        .max_w(px(460.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .px(px(28.))
        .py(px(24.))
        .child(ui::emblem(seen.icon.icon()))
        .child(heading(&text.title))
        .child(lead_text(text.line.clone()))
        .child(div().text_center().text_size(px(12.)).text_color(colours::faint()).child(text.folder.clone()));
    div().absolute().inset_0().flex().items_center().justify_center().p(px(24.)).child(card)
}

impl Root {
    pub(crate) fn machine_place_changed(
        &mut self,
        place: Option<MachinePlace>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.side.machine_place = place;
        self.activation_changed(window.is_window_active(), cx);
        cx.notify();
    }

    pub(super) fn machine_tab_area(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let place = self.side.machine_place.as_ref()?;
        let seen = self.session.as_ref()?.machine(&place.machine)?;
        let text = card_text(place, seen, self.store.read(cx).machine_home(&place.boot));
        Some(machine_card(&text, seen).into_any_element())
    }

    pub(crate) fn typed_on_machine(&mut self, cx: &mut Context<Self>) {
        let Some(machine) = self.side.machine_place.as_ref().map(|place| place.machine.clone()) else { return };
        let text = format!("The tab on {machine} is not drawn here yet: nothing was sent");
        self.store.update(cx, |s, cx| s.toast(text, ToastKind::Info, cx));
    }

    pub(crate) fn keep_seen_counting(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| keep_counting(this, cx).await).detach();
    }

    fn count_seen_up(&mut self, cx: &mut Context<Self>) {
        if self.session.as_ref().is_some_and(|session| session.machines.iter().any(MachineSeen::counts_up)) {
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests;
