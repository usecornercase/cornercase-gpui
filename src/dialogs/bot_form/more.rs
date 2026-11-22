use gpui::{
    App, ClickEvent, Context, Div, FocusHandle, FontWeight, InteractiveElement as _, ParentElement as _, Stateful,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::{BotForm, Mode};
use crate::model::bots::{AgentInfo, FollowRow, Row};
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, press};

const MORE: &str = "More options";
const STOP: &str = "bot-more";

pub(super) struct More {
    pub(super) open: bool,
    stop: FocusHandle,
}

impl More {
    pub(super) fn new(mode: &Mode, cx: &mut App) -> Self {
        Self { open: matches!(mode, Mode::Edit(_)), stop: cx.focus_handle() }
    }
}

impl BotForm {
    fn more_pressed(&mut self, event: &ClickEvent, cx: &mut Context<Self>) {
        if press::pressed(event, STOP) {
            self.more.open = !self.more.open;
            cx.notify();
        }
    }

    fn more_summary(&self, agent: &AgentInfo, cx: &App) -> String {
        let model = self.model.read(cx).text();
        let mut parts = vec![if model.trim().is_empty() { "Model".to_string() } else { model.trim().to_string() }];
        if agent.takes_effort() {
            parts.push(self.effort.as_ref().map_or_else(|| "Effort".to_string(), |e| format!("{e} effort")));
        }
        let store = Store::get(cx).read(cx);
        if !matches!(store.follow_row(), FollowRow::Hidden) {
            parts.push("Follow agents".to_string());
        }
        if !matches!(store.chat_apps_row(), Row::Hidden) {
            parts.push("Chat apps".to_string());
        }
        parts.join(" · ")
    }

    pub(super) fn more_head(&self, agent: &AgentInfo, cx: &mut Context<Self>) -> Stateful<Div> {
        let open = self.more.open;
        let glyph = if open { Icon::ChevronDown } else { Icon::ChevronRight };
        let summary = (!open).then(|| {
            let words = self.more_summary(agent, cx);
            div().flex_1().min_w_0().text_size(px(11.)).text_color(colours::faint()).child(ui::one_line(words))
        });
        let head = div()
            .id(STOP)
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .py(px(7.))
            .mx(px(-8.))
            .rounded(px(radius::control()))
            .cursor_pointer()
            .hover(|s| s.bg(colours::hover()))
            .child(ui::icon(glyph, 12.).text_color(colours::muted()))
            .child(div().flex_none().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(MORE))
            .when_some(summary, gpui::ParentElement::child);
        press::stop_at(head, &self.more.stop, STOP, true)
            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| this.more_pressed(event, cx)))
    }
}
