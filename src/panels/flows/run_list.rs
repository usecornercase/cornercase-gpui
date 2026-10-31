use std::collections::HashSet;
use std::ops::Range;

use gpui::{
    AnyElement, Context, IntoElement, ListAlignment, ListState, ParentElement as _, Styled as _, div, list,
    prelude::FluentBuilder as _, px,
};

use super::FlowsPanel;
use super::model::Run;

const OVERDRAW: f32 = 600.;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Part {
    Top,
    Feed,
    Message(u64),
    Ended,
    Files,
}

pub(super) struct RunList {
    list: ListState,
    run: Option<u64>,
    parts: Vec<Part>,
}

impl RunList {
    pub(super) fn new() -> Self {
        Self { list: ListState::new(0, ListAlignment::Top, px(OVERDRAW)), run: None, parts: Vec::new() }
    }

    pub(super) fn sync(&mut self, run: Option<u64>, parts: Vec<Part>) {
        if run != self.run {
            self.list.reset(parts.len());
        } else if let Some(splices) = splices(&self.parts, &parts) {
            for (range, count) in splices {
                self.list.splice(range, count);
            }
        } else {
            let top = self.list.logical_scroll_top();
            self.list.reset(parts.len());
            self.list.scroll_to(top);
        }
        self.run = run;
        self.parts = parts;
    }
}

fn splices(old: &[Part], new: &[Part]) -> Option<Vec<(Range<usize>, usize)>> {
    let (olds, news): (HashSet<_>, HashSet<_>) = (old.iter().collect(), new.iter().collect());
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < old.len() || j < new.len() {
        if i < old.len() && j < new.len() && old[i] == new[j] {
            (i, j) = (i + 1, j + 1);
            continue;
        }
        let gone = old[i..].iter().take_while(|p| !news.contains(p)).count();
        let added = new[j..].iter().take_while(|p| !olds.contains(p)).count();
        if gone == 0 && added == 0 {
            return None;
        }
        out.push((j..j + gone, added));
        (i, j) = (i + gone, j + added);
    }
    Some(out)
}

impl FlowsPanel {
    pub(super) fn sync_run_list(&mut self) {
        let run = self.selected_run();
        let parts = run.map(|run| self.run_parts(run)).unwrap_or_default();
        let id = run.map(|r| r.id);
        self.run_list.sync(id, parts);
    }

    fn run_parts(&self, run: &Run) -> Vec<Part> {
        let live = !run.closed();
        let messages = self.feed.messages.iter().rev().filter(|_| live).map(|m| Part::Message(m.seq));
        let ended = self.feed.ended.as_ref().filter(|_| live).map(|_| Part::Ended);
        [Part::Top, Part::Feed].into_iter().chain(messages).chain(ended).chain([Part::Files]).collect()
    }

    pub(super) fn render_run(&self, cx: &Context<Self>) -> AnyElement {
        let render = cx.processor(|this: &mut Self, index: usize, _, cx| this.run_part(index, cx));
        list(self.run_list.list.clone(), render).size_full().into_any_element()
    }

    fn run_part(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let parts = &self.run_list.parts;
        let (Some(run), Some(&part)) = (self.selected_run(), parts.get(index)) else {
            return div().into_any_element();
        };
        let before = match part {
            Part::Top => 20.,
            Part::Feed | Part::Files => 22.,
            Part::Message(_) | Part::Ended => 0.,
        };
        div()
            .w_full()
            .px(px(20.))
            .pt(px(before))
            .when(index + 1 == parts.len(), |d| d.pb(px(32.)))
            .when(part == Part::Feed && matches!(parts.get(index + 1), Some(Part::Message(_))), |d| d.pb(px(10.)))
            .child(self.part(run, part, cx))
            .into_any_element()
    }

    fn part(&self, run: &Run, part: Part, cx: &Context<Self>) -> AnyElement {
        match part {
            Part::Top => self.run_top(run, cx).into_any_element(),
            Part::Feed => self.feed_heading(run).into_any_element(),
            Part::Message(seq) => self.message_part(run, seq, cx),
            Part::Ended => self.feed_ended(),
            Part::Files => self.files_block(run).into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests;
