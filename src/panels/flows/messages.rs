use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, Div, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Rgba, Stateful, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
    px, rgba,
};

use super::FlowsPanel;
use super::cards::{avatar, conductor, ground};
use super::diagram::role_colour;
use super::flow_page::block;
use super::model::{self, Edge, FlowSpec, Message, Run, RunFile};
use super::widgets::{chip, error_small, open_path, panel_box};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone, glass};

const PREVIEW: usize = 1200;
const DOT: f32 = 12.;
const DOT_TOP: f32 = 15.;

impl FlowsPanel {
    pub(super) fn feed_heading(&self, run: &Run) -> Div {
        let heading = block(Icon::Message, &format!("Messages · {}", self.feed.messages.len()));
        if run.closed() {
            return heading
                .child(quiet("Messages are kept while the run is live; reopen the agents to follow it again."));
        }
        heading.when(self.feed.messages.is_empty(), |d| {
            d.child(quiet(
                "Nothing sent yet. The orchestrator's sends, keys, reads and waits appear here as they happen.",
            ))
        })
    }

    pub(super) fn message_part(&self, run: &Run, seq: u64, cx: &Context<Self>) -> AnyElement {
        let messages = &self.feed.messages;
        let Ok(at) = messages.binary_search_by_key(&seq, |m| m.seq) else { return div().into_any_element() };
        let message = &messages[at];
        div()
            .flex()
            .gap(px(12.))
            .child(rail(role_colour(&run.flow, &message.to), (at + 1 == messages.len(), at == 0)))
            .child(self.bubble(run, message, model::now(), cx).mb(px(10.)))
            .into_any_element()
    }

    pub(super) fn feed_ended(&self) -> AnyElement {
        match &self.feed.ended {
            Some(ended) => ui::dim(format!("Not following: {ended}")).into_any_element(),
            None => div().into_any_element(),
        }
    }

    pub(super) fn files_block(&self, run: &Run) -> Div {
        block(Icon::File, "Files").child(self.files_list(run))
    }

    fn bubble(&self, run: &Run, message: &Message, now: i64, cx: &Context<Self>) -> Stateful<Div> {
        let seq = message.seq;
        let whole = self.expanded.contains(&seq);
        let text = message.text.clone().unwrap_or_default();
        let shown: String = if whole { text.clone() } else { text.chars().take(PREVIEW).collect() };
        let cut = u64::try_from(text.len()).is_ok_and(|len| message.bytes > len);
        panel_box()
            .id(("message", seq))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .cursor_pointer()
            .hover(|s| s.border_color(colours::edge()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_message(seq, cx)))
            .child(header(&run.flow, message, text, now))
            .when(!shown.is_empty(), |d| d.child(body(shown, whole)))
            .when(cut && whole, |d| d.child(ui::dim(format!("cut at 32 KiB of {}", model::size(message.bytes)))))
    }

    fn toggle_message(&mut self, seq: u64, cx: &mut Context<Self>) {
        if !self.expanded.remove(&seq) {
            self.expanded.insert(seq);
        }
        cx.notify();
    }

    fn files_list(&self, run: &Run) -> Div {
        let list = panel_box().flex().flex_col().gap(px(1.)).p(px(6.)).children(run.dir.clone().map(folder_line));
        match &self.files {
            None => list.child(ui::busy("folder-loading", "Reading the folder…").px(px(8.)).py(px(6.))),
            Some(Err(e)) => list.child(error_small(e.clone()).p(px(8.))),
            Some(Ok(files)) if files.entries.is_empty() => {
                list.child(quiet("No files yet: the roles write their answers here."))
            }
            Some(Ok(files)) => {
                let now = model::now();
                list.children(files.entries.iter().enumerate().map(|(i, file)| file_row(i, file, now)))
                    .when(files.more, |d| d.child(quiet("Only the newest 200 files are listed.")))
            }
        }
    }
}

fn quiet(text: &'static str) -> Div {
    ui::dim(text).px(px(8.)).py(px(6.))
}

fn rail(colour: Rgba, (first, last): (bool, bool)) -> Div {
    let centre = DOT_TOP + DOT / 2.;
    let top = if first { centre } else { 0. };
    let line = div().absolute().left(px(DOT / 2. - 1.)).top(px(top)).w(px(2.)).bg(rgba(0xff_ff_ff_14));
    div()
        .relative()
        .w(px(DOT))
        .flex_none()
        .when(!(first && last), |d| d.child(if last { line.h(px(centre - top)) } else { line.bottom_0() }))
        .child(
            div()
                .absolute()
                .top(px(DOT_TOP))
                .size(px(DOT))
                .rounded_full()
                .bg(colour)
                .border_2()
                .border_color(ground())
                .shadow(glass::glow(colour, 0.5, 8.)),
        )
}

fn who(flow: &FlowSpec, role: &str) -> Div {
    let colour = role_colour(flow, role);
    let face = if flow.is_lead(role) { conductor(18.) } else { avatar(role, colour, 18.) };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .child(face)
        .child(div().font_weight(FontWeight::SEMIBOLD).text_color(colour).child(role.to_string()))
}

fn edge_chip(flow: &FlowSpec, edge: Option<Edge>) -> Option<Div> {
    let (glyph, text, colour) = match edge? {
        Edge::Forward { stage } => (Icon::ArrowDown, format!("stage {} → {}", stage + 1, stage + 2), colours::accent()),
        Edge::Loop { stage } => (Icon::Repeat, flow.stages.get(stage)?.back.as_ref()?.when.clone(), colours::soft()),
        Edge::Outside { .. } => (Icon::Alert, "outside the plan".to_string(), colours::waiting()),
    };
    Some(chip(Some(glyph), text, colour))
}

fn header(flow: &FlowSpec, message: &Message, text: String, now: i64) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(12.))
        .child(who(flow, &message.from))
        .child(ui::icon(Icon::ChevronRight, 12.).text_color(colours::faint()))
        .child(who(flow, &message.to))
        .child(div().text_color(colours::muted()).child(message.verb().to_string()))
        .children(edge_chip(flow, message.edge))
        .child(div().flex_1())
        .child(ui::dim(format!("#{} · {}", message.seq, model::ago(&message.time, now))))
        .when(!text.is_empty(), |d| d.child(copy_button(message.seq, text)))
}

fn copy_button(seq: u64, text: String) -> Stateful<Div> {
    div()
        .id(("copy-message", seq))
        .flex_none()
        .p(px(4.))
        .rounded(px(radius::CHIP))
        .cursor_pointer()
        .hover(|s| s.bg(colours::selected()))
        .child(ui::icon(Icon::Copy, 12.).text_color(colours::faint()))
        .on_click(move |_: &ClickEvent, _, cx| {
            cx.stop_propagation();
            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        })
}

fn body(shown: String, whole: bool) -> Div {
    let text = if whole { div().line_height(px(19.)) } else { ui::clamped(4, 19.) };
    text.text_size(px(12.5)).text_color(colours::soft()).child(shown)
}

fn folder_line(dir: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .py(px(6.))
        .mb(px(2.))
        .child(ui::icon(Icon::FolderOpen, 14.).text_color(colours::accent()))
        .child(
            div().flex_1().min_w_0().text_size(px(11.)).text_color(colours::muted()).child(ui::one_line(dir.clone())),
        )
        .child(
            ui::icon_text_button("run-folder", Icon::FolderOpen, "Open folder", Tone::Plain).on_click(open_path(dir)),
        )
}

fn file_row(i: usize, file: &RunFile, now: i64) -> Stateful<Div> {
    let age = file.modified.as_deref().map(|m| model::ago(m, now)).unwrap_or_default();
    ui::row(("run-file", i), false)
        .on_click(open_path(file.path.clone()))
        .child(ui::icon(Icon::File, 14.).text_color(colours::muted()))
        .child(div().flex_1().min_w_0().text_size(px(12.)).child(ui::one_line(file.name.clone())))
        .child(ui::dim(model::size(file.bytes)).w(px(64.)).flex_none())
        .child(ui::dim(age).w(px(36.)).flex_none())
}
