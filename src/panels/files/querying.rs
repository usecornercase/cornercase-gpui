use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use gpui::{AppContext as _, Context, ScrollStrategy};

use super::source::Source;
use super::viewer::OpenAt;
use super::{FilesPanel, INDEX_FRESH, Mode, Results, TextRow, search, served};

pub(super) struct Job {
    pub(super) root: PathBuf,
    pub(super) source: Source,
    pub(super) index: Option<Arc<Vec<String>>>,
    pub(super) query: String,
    pub(super) mode: Mode,
    pub(super) cancel: Arc<AtomicBool>,
}

pub(super) struct Answer {
    pub(super) root: PathBuf,
    pub(super) paths: Arc<Vec<String>>,
    pub(super) fresh: bool,
    query: String,
    pub(super) results: Results,
}

impl Job {
    pub(super) fn spawn<T: 'static>(
        self,
        cx: &mut Context<T>,
        then: impl FnOnce(&mut T, Answer, &mut Context<T>) + 'static,
    ) {
        let task = cx.background_spawn(async move { answer(self) });
        cx.spawn(async move |this, cx| {
            let answer = task.await;
            let _ = this.update(cx, |this, cx| then(this, answer, cx));
        })
        .detach();
    }
}

fn answer(job: Job) -> Answer {
    if let Source::Server(workspace) = job.source {
        let results = served::search(workspace, &job.query, job.mode);
        return Answer { root: job.root, paths: Arc::default(), fresh: false, query: job.query, results };
    }
    let (paths, fresh) = match job.index {
        Some(paths) => (paths, false),
        None => search::index(&job.root, &job.cancel).map_or_else(|| (Arc::default(), false), |p| (Arc::new(p), true)),
    };
    let results = match job.mode {
        Mode::Names => Results::Names(search::names(&paths, &job.query)),
        Mode::Text => Results::Text(search::text(&job.root, &paths, &job.query, &job.cancel)),
    };
    Answer { root: job.root, paths, fresh, query: job.query, results }
}

fn text_rows(text: &search::TextResults) -> Vec<TextRow> {
    let mut rows = Vec::new();
    for (f, file) in text.files.iter().enumerate() {
        rows.push(TextRow::File(f));
        rows.extend((0..file.lines.len()).map(|l| TextRow::Line(f, l)));
    }
    rows
}

fn text_target(text: &search::TextResults, row: Option<&TextRow>) -> Option<(String, Option<u64>)> {
    match row? {
        TextRow::File(f) => text.files.get(*f).map(|f| (f.path.clone(), f.lines.first().map(|l| l.line))),
        TextRow::Line(f, l) => text.files.get(*f).and_then(|f| f.lines.get(*l).map(|l| (f.path.clone(), Some(l.line)))),
    }
}

fn zero_based(line: u64) -> usize {
    usize::try_from(line).unwrap_or(usize::MAX).saturating_sub(1)
}

impl FilesPanel {
    pub(super) fn query_changed(&mut self, text: String, cx: &mut Context<Self>) {
        if let Some(place) = self.place_mut() {
            place.query = text;
            place.viewer = None;
        }
        self.search.enter = false;
        self.run_search(cx);
    }

    pub(super) fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.search.mode != mode {
            self.search.mode = mode;
            self.search.results = Results::None;
            self.search.rows.clear();
            self.run_search(cx);
        }
    }

    pub(super) fn fresh_index(&self, root: &Path) -> Option<Arc<Vec<String>>> {
        self.search
            .index
            .as_ref()
            .filter(|(r, _, at)| r.as_path() == root && at.elapsed() < INDEX_FRESH)
            .map(|(_, paths, _)| paths.clone())
    }

    pub(super) fn keep_index(&mut self, root: PathBuf, paths: Arc<Vec<String>>) {
        if self.place().is_some_and(|p| p.root == root) {
            self.search.index = Some((root, paths, Instant::now()));
        }
    }

    pub(super) fn run_search(&mut self, cx: &mut Context<Self>) {
        if let Some(running) = self.search.running.take() {
            running.store(true, Ordering::Relaxed);
        }
        self.search.generation += 1;
        let query = self.place().map(|p| p.query.trim().to_string()).unwrap_or_default();
        let Some((root, source)) = self.place().map(|p| (p.root.clone(), p.source.clone())) else { return };
        if query.is_empty() {
            self.search.clear();
            cx.notify();
            return;
        }
        let generation = self.search.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.search.running = Some(cancel.clone());
        let job = Job { index: self.fresh_index(&root), root, source, query, mode: self.search.mode, cancel };
        job.spawn(cx, move |this, answer, cx| this.answered(generation, answer, cx));
        cx.notify();
    }

    fn answered(&mut self, generation: u64, answer: Answer, cx: &mut Context<Self>) {
        if answer.fresh {
            self.keep_index(answer.root, answer.paths);
        }
        if self.search.generation != generation {
            return;
        }
        self.search.running = None;
        self.search.answered = answer.query;
        self.search.rows = match &answer.results {
            Results::Text(text) => text_rows(text),
            _ => Vec::new(),
        };
        self.search.results = answer.results;
        self.results_scroll.scroll_to_item(0, ScrollStrategy::Top);
        if std::mem::take(&mut self.search.enter) {
            self.open_result(0, cx);
        }
        cx.notify();
    }

    pub(super) fn enter(&mut self, cx: &mut Context<Self>) {
        if self.search.running.is_some() {
            self.search.enter = true;
        } else {
            self.open_result(0, cx);
        }
    }

    pub(super) fn open_result(&mut self, ix: usize, cx: &mut Context<Self>) {
        let (rel, at) = match &self.search.results {
            Results::None | Results::Failed(_) => return,
            Results::Names(hits) => {
                let Some(hit) = hits.get(ix) else { return };
                (PathBuf::from(&hit.path), OpenAt { from_search: true, ..OpenAt::default() })
            }
            Results::Text(text) => {
                let Some((path, line)) = text_target(text, self.search.rows.get(ix)) else { return };
                let lit = search::matcher(&self.search.answered);
                (PathBuf::from(path), OpenAt { line: line.map(zero_based), lit, from_search: true })
            }
        };
        self.open_file(rel, at, cx);
    }
}

#[cfg(test)]
mod tests;
