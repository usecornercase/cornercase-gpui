use gpui::Context;

use super::data::{self, Agents, Issue, Lists, People, ReadIssue, SourceInfo, Sources, Who};
use super::{IssuesPanel, ListKey, ListState, Tab, ask, first_line, page};
use crate::store::args;

impl IssuesPanel {
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.project else { return };
        self.loading_sources = true;
        self.lists.clear();
        let selected = self.selected.clone();
        self.reads.retain(|id, _| selected.as_ref().is_some_and(|s| s.id() == *id));
        self.people_lists.clear();
        if let Some(issue) = &selected {
            self.read(issue, cx);
        }
        let first = self.sources.as_ref().is_none_or(|s| s.project != project);
        ask(cx, args(&["issue", "sources", "--json", "--project", &project.to_string()]), move |this, out, cx| {
            this.sources_answered(project, first, out, cx);
        });
        if self.agents.is_none() {
            ask(cx, args(&["config", "--json"]), Self::config_answered);
        }
        cx.notify();
    }

    fn sources_answered(&mut self, project: u64, first: bool, out: Result<String, String>, cx: &mut Context<Self>) {
        if self.project != Some(project) {
            return;
        }
        self.loading_sources = false;
        match out.and_then(|out| data::parse::<Sources>(&out).ok_or_else(|| "cornercase answered no sources".into())) {
            Ok(sources) => {
                if first {
                    self.take_filters(&sources);
                }
                self.sources_error = None;
                self.sources = Some(sources);
                self.load_list(true, cx);
            }
            Err(e) => self.sources_error = Some(e),
        }
        cx.notify();
    }

    fn take_filters(&mut self, sources: &Sources) {
        self.closed = sources.closed;
        self.people = sources.list.iter().map(|s| (Tab::One(s.source.clone()), s.people.clone())).collect();
        let keep = self.tab.source().is_some_and(|s| sources.get(s).is_some_and(|s| s.tab));
        if !keep {
            let connected: Vec<_> = sources.tabs().into_iter().filter(|s| s.connected()).collect();
            self.tab = match connected.as_slice() {
                [one] => Tab::One(one.source.clone()),
                _ => Tab::All,
            };
        }
    }

    fn config_answered(&mut self, out: Result<String, String>, cx: &mut Context<Self>) {
        let config = out.and_then(|o| serde_json::from_str::<serde_json::Value>(&o).map_err(|e| e.to_string()));
        let Ok(value) = config else { return };
        self.agents = Some(Agents::from_config(&value));
        cx.notify();
    }

    pub(super) fn who(&self, tab: &Tab) -> [Who; 2] {
        self.people.get(tab).cloned().unwrap_or_default()
    }

    pub(super) fn fields(&self) -> [String; 2] {
        match self.tab.source().and_then(|s| self.source(s)) {
            Some(source) => source.fields.clone(),
            None => ["assignee".to_string(), "author".to_string()],
        }
    }

    pub(super) fn list_key(&self) -> Option<ListKey> {
        let people = self.who(&self.tab);
        Some(ListKey {
            project: self.project?,
            tab: self.tab.clone(),
            closed: self.closed,
            people: [people[0].arg(), people[1].arg()],
        })
    }

    pub(super) fn source(&self, source: &str) -> Option<&SourceInfo> {
        self.sources.as_ref()?.get(source)
    }

    pub(super) fn source_name(&self, source: &str) -> String {
        self.source(source).map_or_else(|| source.to_string(), |s| s.name.clone())
    }

    pub(super) fn current(&self) -> Option<&ListState> {
        self.lists.get(&self.list_key()?)
    }

    pub(super) fn shown(&self, cx: &Context<Self>) -> Vec<Issue> {
        let query = self.search.read(cx).text();
        self.current().map(|l| l.issues.iter().filter(|i| i.matches(&query)).cloned().collect()).unwrap_or_default()
    }

    fn can_list(&self) -> bool {
        match self.tab.source() {
            Some(source) => self.source(source).is_some_and(SourceInfo::connected),
            None => self.sources.as_ref().is_some_and(Sources::any_connected),
        }
    }

    pub(super) fn load_list(&mut self, cached: bool, cx: &mut Context<Self>) {
        let Some(key) = self.list_key() else { return };
        if !self.can_list() {
            return;
        }
        self.lists.entry(key.clone()).or_default().loading = true;
        let command = self.list_args(&key, cached);
        ask(cx, command, move |this, out, cx| this.list_answered(&key, cached, out, cx));
        cx.notify();
    }

    fn list_args(&self, key: &ListKey, cached: bool) -> Vec<String> {
        let mut a = args(&["issue", "list", "--json", "--project", &key.project.to_string()]);
        if let Some(source) = key.tab.source() {
            a.extend(["--source".into(), source.into()]);
        }
        if key.closed {
            a.push("--closed".into());
        }
        let who = self.who(&key.tab);
        for (flag, who) in ["--assignee", "--author"].into_iter().zip(&who) {
            if *who != Who::Anyone && (key.tab != Tab::All || !who.is_person()) {
                a.extend([flag.into(), who.arg()]);
            }
        }
        if cached {
            a.push("--cached".into());
        }
        a
    }

    fn list_answered(&mut self, key: &ListKey, cached: bool, out: Result<String, String>, cx: &mut Context<Self>) {
        let state = self.lists.entry(key.clone()).or_default();
        match out.and_then(|out| data::parse::<Lists>(&out).ok_or_else(|| first_line(&out))) {
            Ok(lists) => state.take(lists, cached),
            Err(e) => state.fail(e, cached),
        }
        state.cached = cached;
        state.loading = cached;
        if cached {
            self.load_list(false, cx);
        }
        cx.notify();
    }

    pub(super) fn select(&mut self, issue: Issue, cx: &mut Context<Self>) {
        let id = issue.id();
        if !self.reads.contains_key(&id) {
            self.read(&issue, cx);
        }
        if self.selected.as_ref().is_none_or(|s| s.id() != id)
            && let Some(Ok(page)) = self.reads.get(&id)
        {
            page.to_top();
        }
        self.selected = Some(issue);
        cx.notify();
    }

    fn read(&mut self, issue: &Issue, cx: &mut Context<Self>) {
        let Some(project) = self.project else { return };
        let id = issue.id();
        if !self.reading.insert(id.clone()) {
            return;
        }
        let command = args(&[
            "issue",
            "read",
            &issue.key,
            "--source",
            &issue.source,
            "--json",
            "--project",
            &project.to_string(),
        ]);
        ask(cx, command, move |this, out, cx| {
            this.reading.remove(&id);
            let read = out.and_then(|out| data::parse::<ReadIssue>(&out).ok_or_else(|| first_line(&out)));
            page::take(&mut this.reads, id, read);
            cx.notify();
        });
    }

    pub(super) fn load_people(&mut self, source: String, cx: &mut Context<Self>) {
        let Some(project) = self.project else { return };
        if self.people_lists.contains_key(&source) {
            return;
        }
        let command = args(&["issue", "people", &source, "--json", "--project", &project.to_string()]);
        ask(cx, command, move |this, out, cx| {
            let people = out.and_then(|out| data::parse::<People>(&out).ok_or_else(|| first_line(&out)));
            this.people_lists.insert(source, people);
            cx.notify();
        });
    }
}

impl ListState {
    fn take(&mut self, mut lists: Lists, cached: bool) {
        data::sort_newest_first(&mut lists.issues);
        if !(cached && lists.issues.is_empty() && self.loaded) {
            self.issues = lists.issues;
            self.sources = lists.sources;
        }
        self.loaded = self.loaded || !cached || !self.issues.is_empty();
        self.error = None;
    }

    fn fail(&mut self, error: String, cached: bool) {
        if !cached {
            self.error = Some(error);
            self.loaded = true;
        }
    }
}
