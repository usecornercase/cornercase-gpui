use gpui::{ClipboardItem, Context, ListAlignment, ListOffset, ListState, Window, px};

use super::QueuePanel;
use super::data::Entry;
use super::read::{Ask, Keep, Reads, failed, job_key, job_words, show_args};
use super::result::{Answer, Page, Talk, copied, merged, page_after, shown_lines, with_talk};
use crate::cli;
use crate::store::{self, Busy, Store, ToastKind};
use crate::ui::selection::Selecting;

const OVERDRAW: f32 = 600.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum View {
    Rendered,
    Text,
}

pub(super) struct Lists {
    message: ListState,
    message_text: ListState,
    talk: ListState,
    talk_text: ListState,
}

fn fresh() -> ListState {
    ListState::new(0, ListAlignment::Top, px(OVERDRAW))
}

fn resized(list: &ListState, count: usize, keep: bool) {
    let top = list.logical_scroll_top();
    list.reset(count);
    if keep {
        list.scroll_to(top);
    } else {
        list.scroll_to(ListOffset::default());
    }
}

impl Lists {
    fn new() -> Self {
        Self { message: fresh(), message_text: fresh(), talk: fresh(), talk_text: fresh() }
    }

    pub(super) fn of(&self, page: Page, view: View) -> &ListState {
        match (page, view) {
            (Page::Message, View::Rendered) => &self.message,
            (Page::Message, View::Text) => &self.message_text,
            (Page::Conversation, View::Rendered) => &self.talk,
            (Page::Conversation, View::Text) => &self.talk_text,
        }
    }
}

pub(super) struct Detail {
    pub(super) id: u64,
    pub(super) label: String,
    reads: Reads,
    pub(super) answer: Option<Answer>,
    pub(super) error: Option<String>,
    pub(super) page: Page,
    pub(super) view: View,
    pub(super) lists: Lists,
    pub(super) selecting: Selecting<usize>,
}

impl Detail {
    fn new(entry: &Entry, view: View, base: u64) -> (Self, Ask) {
        let (reads, ask) = Reads::new(base, entry.result_at);
        let detail = Self {
            id: entry.id,
            label: entry.label.clone(),
            reads,
            answer: None,
            error: None,
            page: Page::Message,
            view,
            lists: Lists::new(),
            selecting: Selecting::default(),
        };
        (detail, ask)
    }

    pub(super) fn moved(&mut self, entry: &Entry) -> Option<Ask> {
        self.label.clone_from(&entry.label);
        self.reads.moved(entry.result_at)
    }

    pub(super) fn lines(&self) -> &[String] {
        self.answer.as_ref().map_or(&[], |answer| shown_lines(answer, self.page))
    }

    fn relist(&mut self, last_changed: bool) {
        let Some(answer) = self.answer.as_ref() else { return };
        resized(&self.lists.message, answer.blocks.len(), !last_changed);
        resized(&self.lists.message_text, answer.lines.len(), !last_changed);
        let (blocks, lines) = match &answer.talk {
            Some(Talk::Said(said)) => (said.messages.len(), said.lines.len()),
            _ => (0, 0),
        };
        resized(&self.lists.talk, blocks, true);
        resized(&self.lists.talk_text, lines, true);
    }

    fn take(&mut self, answer: Answer, keep: Keep) {
        let last_changed = self.answer.as_ref().is_none_or(|old| old.lines != answer.lines);
        let (answer, changed) = merged(self.answer.take(), answer, keep);
        self.page = page_after(&answer, self.page);
        self.answer = Some(answer);
        self.error = None;
        self.refreshed(changed, last_changed);
    }

    fn take_talk_only(&mut self, answer: Answer) {
        let Some(shown) = self.answer.take() else { return };
        let (shown, changed) = with_talk(shown, answer);
        self.page = page_after(&shown, self.page);
        self.answer = Some(shown);
        self.refreshed(changed, false);
    }

    fn refreshed(&mut self, changed: bool, last_changed: bool) {
        if changed {
            self.selecting.clear();
        }
        self.relist(last_changed);
    }
}

fn toast(text: String, cx: &mut Context<QueuePanel>) {
    Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Info, cx));
}

impl QueuePanel {
    pub(super) fn open_detail(&mut self, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) {
        self.form = None;
        if self.detail.as_ref().is_none_or(|detail| detail.id != entry.id) {
            let view = self.detail.as_ref().map_or(View::Rendered, |detail| detail.view);
            let (detail, ask) = Detail::new(entry, view, self.seqs.base());
            self.detail = Some(detail);
            self.ask(entry.id, ask, cx);
        }
        window.focus(&self.focus);
        cx.notify();
    }

    pub(super) fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.detail = None;
        cx.notify();
    }

    pub(super) fn ask(&mut self, id: u64, ask: Ask, cx: &mut Context<Self>) {
        let Some(detail) = self.detail.as_ref().filter(|detail| detail.id == id) else { return };
        self.seqs.seen(ask);
        let busy = Busy::new(job_key(id, ask.talk), job_words(&detail.label, ask.talk));
        let command = show_args(id, ask.talk);
        let work = move || cli::run_partial(&command).and_then(|out| Answer::read(&out, ask.talk));
        store::run_local_busy(cx, busy, work, move |this, result, cx| this.answered(id, ask, result, cx));
    }

    fn answered(&mut self, id: u64, ask: Ask, result: anyhow::Result<Answer>, cx: &mut Context<Self>) {
        let Some(detail) = self.detail.as_mut().filter(|detail| detail.id == id) else { return };
        let keep = detail.reads.answered(ask);
        match result {
            Ok(answer) if keep.facts => detail.take(answer, keep),
            Ok(answer) if keep.talk => detail.take_talk_only(answer),
            Err(e) if keep.facts || keep.talk => detail.error = Some(failed(ask.talk, &cli::reason(&e))),
            Ok(_) | Err(_) => {}
        }
        cx.notify();
    }

    pub(super) fn choose_page(&mut self, page: Page, cx: &mut Context<Self>) {
        let Some(detail) = self.detail.as_mut() else { return };
        detail.page = page;
        detail.selecting.clear();
        let (id, ask) = (detail.id, (page == Page::Conversation).then(|| detail.reads.wants_talk()).flatten());
        if let Some(ask) = ask {
            self.ask(id, ask, cx);
        }
        cx.notify();
    }

    pub(super) fn choose_view(&mut self, view: View, cx: &mut Context<Self>) {
        if let Some(detail) = self.detail.as_mut() {
            detail.view = view;
            detail.selecting.clear();
        }
        cx.notify();
    }

    pub(super) fn try_again(&mut self, cx: &mut Context<Self>) {
        let Some(detail) = self.detail.as_mut() else { return };
        detail.error = None;
        let (id, ask) = (detail.id, detail.reads.again());
        self.ask(id, ask, cx);
        cx.notify();
    }

    pub(super) fn copy_shown(&mut self, cx: &mut Context<Self>) {
        let Some((answer, page)) = self.detail.as_ref().and_then(|d| Some((d.answer.as_ref()?, d.page))) else {
            return;
        };
        let Some(text) = copied(answer, page) else { return };
        let what = if page == Page::Message { "the last message" } else { "the conversation" };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        toast(format!("Copied {what}"), cx);
    }

    pub(super) fn sync_detail(&mut self, entries: &[Entry], results: bool, cx: &mut Context<Self>) {
        let Some(id) = self.detail.as_ref().map(|detail| detail.id) else { return };
        let entry = entries.iter().find(|e| e.id == id).filter(|_| results);
        let Some(entry) = entry else {
            self.detail = None;
            return;
        };
        let ask = self.detail.as_mut().and_then(|detail| detail.moved(entry));
        if let Some(ask) = ask {
            cx.spawn(async move |this, cx| {
                let _ = this.update(cx, |this, cx| this.ask(id, ask, cx));
            })
            .detach();
        }
    }
}
