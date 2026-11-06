mod agents;
mod data;
mod limits;
mod machines;
mod parts;
mod stats;
mod widgets;

use std::collections::HashMap;
use std::time::Duration;

use gpui::{
    AnyElement, AnyView, AppContext as _, AsyncApp, ClickEvent, ClipboardItem, Context, Div, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Stateful, StatefulInteractiveElement as _,
    Styled as _, Subscription, Task, WeakEntity, Window, div, prelude::FluentBuilder as _, px,
};

use self::data::{Plans, Stats, WINDOWS};
use self::machines::{Link, Pick};
use self::widgets::{card, failure, loading, notes, plural, stale};
use crate::cli;
use crate::model::machines::THIS_COMPUTER;
use crate::panels::PanelKind;
use crate::store::{self, Store, ToastKind};
use crate::theme::colours;
use crate::ui;

const EMPTY: &str = "nothing counted yet: agents running in cornercase count from now on";
const TICK: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Limits,
    Stats,
    Agents,
}

impl Page {
    const ALL: [(Page, &'static str); 3] =
        [(Page::Limits, "Limits"), (Page::Stats, "Stats"), (Page::Agents, "By agent")];
}

struct Load<T> {
    value: Option<T>,
    loading: bool,
    error: Option<String>,
    at: Option<i64>,
}

impl<T> Default for Load<T> {
    fn default() -> Self {
        Self { value: None, loading: false, error: None, at: None }
    }
}

impl<T> Load<T> {
    fn done(&mut self, result: anyhow::Result<T>) {
        self.loading = false;
        match result {
            Ok(value) => {
                self.value = Some(value);
                self.error = None;
                self.at = Some(data::now());
            }
            Err(e) => self.error = Some(cli::reason(&e)),
        }
    }

    fn missing(&self, title: &'static str, waiting: String, cx: &Context<UsagePanel>) -> AnyElement {
        match self.error.as_deref().filter(|_| !self.loading) {
            Some(e) => failure(title, e, cx).into_any_element(),
            None => loading(waiting).into_any_element(),
        }
    }
}

#[derive(Default)]
struct Readings {
    stats: Load<Stats>,
    plans: Load<Plans>,
}

pub(crate) struct UsagePanel {
    page: Page,
    window: usize,
    pick: Pick,
    links: Vec<Link>,
    readings: HashMap<Pick, Readings>,
    unread: Readings,
    sharing: bool,
    open: bool,
    _store: Subscription,
    _ticker: Task<()>,
}

impl UsagePanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        let subscription = cx.observe(&store, |this, store, cx| this.store_changed(&store, cx));
        let ticker = cx.spawn(async move |this, cx| tick_while_alive(this, cx).await);
        let mut panel = Self {
            page: Page::Stats,
            window: 0,
            pick: Pick::default(),
            links: machines::links(&store.read(cx).machines),
            readings: HashMap::new(),
            unread: Readings::default(),
            sharing: false,
            open: store.read(cx).sheet == Some(PanelKind::Usage),
            _store: subscription,
            _ticker: ticker,
        };
        panel.refresh(false, cx);
        panel
    }

    fn store_changed(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let (open, links) = {
            let store = store.read(cx);
            (store.sheet == Some(PanelKind::Usage), machines::links(&store.machines))
        };
        let moved = links != self.links;
        let before = self.shown();
        self.links = links;
        let shows_machines = before != Pick::Here || self.shown() != Pick::Here;
        if open && (!self.open || (moved && shows_machines)) {
            self.refresh(false, cx);
        } else if moved {
            cx.notify();
        }
        self.open = open;
    }

    fn shown(&self) -> Pick {
        self.pick.shown(&self.links)
    }

    fn reading(&self) -> &Readings {
        self.readings.get(&self.shown()).unwrap_or(&self.unread)
    }

    fn reading_of(&mut self, pick: &Pick) -> &mut Readings {
        self.readings.entry(pick.clone()).or_default()
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.open {
            cx.notify();
        }
    }

    fn refresh(&mut self, forced: bool, cx: &mut Context<Self>) {
        match self.page {
            Page::Limits => self.load_plans(forced, cx),
            Page::Stats | Page::Agents => self.load_stats(cx),
        }
    }

    fn load_stats(&mut self, cx: &mut Context<Self>) {
        let pick = self.shown();
        let load = &mut self.reading_of(&pick).stats;
        if load.loading {
            return;
        }
        load.loading = true;
        cx.notify();
        run_json(cx, pick.stats(), move |this, result: anyhow::Result<Stats>, cx| {
            this.reading_of(&pick).stats.done(result);
            cx.notify();
        });
    }

    fn load_plans(&mut self, forced: bool, cx: &mut Context<Self>) {
        let pick = self.shown();
        let load = &mut self.reading_of(&pick).plans;
        if load.loading {
            return;
        }
        let cached_first = !forced && load.value.is_none();
        load.loading = true;
        cx.notify();
        if cached_first {
            let cached = pick.clone();
            run_json(cx, pick.usage(true), move |this, result, cx| this.cached_plans(&cached, result, cx));
        }
        run_json(cx, pick.usage(false), move |this, result: anyhow::Result<Plans>, cx| {
            this.reading_of(&pick).plans.done(result);
            cx.notify();
        });
    }

    fn cached_plans(&mut self, pick: &Pick, result: anyhow::Result<Plans>, cx: &mut Context<Self>) {
        let load = &mut self.reading_of(pick).plans;
        if load.value.is_none()
            && let Ok(plans) = result
        {
            load.value = Some(plans);
            cx.notify();
        }
    }

    fn switch(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page == page {
            return;
        }
        self.page = page;
        self.refresh_if_old(cx);
        cx.notify();
    }

    fn choose(&mut self, pick: Pick, cx: &mut Context<Self>) {
        self.pick = pick;
        self.refresh_if_old(cx);
        cx.notify();
    }

    fn refresh_if_old(&mut self, cx: &mut Context<Self>) {
        let reading = self.reading();
        let old = match self.page {
            Page::Limits => reading.plans.value.is_none() || reading.plans.at.is_none_or(|at| data::now() - at > 60),
            Page::Stats | Page::Agents => reading.stats.at.is_none_or(|at| data::now() - at > 30),
        };
        if old {
            self.refresh(false, cx);
        }
    }

    fn share(&mut self, cx: &mut Context<Self>) {
        if self.sharing {
            return;
        }
        self.sharing = true;
        cx.notify();
        store::run(cx, self.shown().share(), |this, result, cx| {
            this.sharing = false;
            let (text, kind) = match result.map(|out| out.trim().to_string()) {
                Ok(link) if !link.is_empty() => {
                    cx.write_to_clipboard(ClipboardItem::new_string(link));
                    ("copied a link to your stats card".to_string(), ToastKind::Done)
                }
                Ok(_) => ("cornercase gave no stats link".to_string(), ToastKind::Error),
                Err(e) => (e.to_string(), ToastKind::Error),
            };
            Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
            cx.notify();
        });
    }

    fn status(&self) -> (bool, String) {
        let reading = self.reading();
        let (loading, at, old) = match self.page {
            Page::Limits => {
                (reading.plans.loading, reading.plans.at, reading.plans.value.as_ref().is_some_and(Plans::old))
            }
            Page::Stats | Page::Agents => {
                (reading.stats.loading, reading.stats.at, reading.stats.value.as_ref().is_some_and(Stats::old))
            }
        };
        let words = if old { data::asked } else { data::updated };
        let status =
            if loading { "loading…".to_string() } else { at.map(|at| words(data::now() - at)).unwrap_or_default() };
        (loading, status)
    }

    fn picker(&self, cx: &Context<Self>) -> Option<Div> {
        let choices = machines::choices(&self.links, &here());
        if choices.is_empty() {
            return None;
        }
        let shown = self.shown();
        let items = choices
            .into_iter()
            .enumerate()
            .map(|(i, (label, pick))| {
                let selected = pick == shown;
                ui::segment(("usage-pick", i), label, selected)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose(pick.clone(), cx)))
                    .into_any_element()
            })
            .collect();
        Some(div().flex().child(ui::segments(items).flex_wrap()))
    }

    fn header(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let tabs = Page::ALL
            .into_iter()
            .map(|(page, label)| {
                ui::segment(("usage-page", page as usize), label, self.page == page)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.switch(page, cx)))
                    .into_any_element()
            })
            .collect();
        let (loading, status) = self.status();
        let can_share = self.page != Page::Limits
            && self.reading().stats.value.as_ref().is_some_and(|s| !s.empty() || s.merged_pull_requests.is_some());
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(ui::segments(tabs))
            .child(div().flex_1())
            .when(!loading, |d| d.child(ui::dim(status)))
            .when(can_share, |d| d.child(self.share_button(cx)))
            .child(if loading {
                ui::busy_button("usage-refresh", "Refreshing…", ui::Tone::Plain)
            } else {
                ui::button("usage-refresh", "Refresh")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(true, cx)))
            })
    }

    fn share_button(&self, cx: &Context<Self>) -> Stateful<Div> {
        if self.sharing {
            return ui::busy_button("usage-share", "Copying…", ui::Tone::Plain);
        }
        ui::button("usage-share", "Copy card link").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.share(cx)))
    }

    fn limits_page(&self, cx: &Context<Self>) -> AnyElement {
        let (shown, load) = (self.shown(), &self.reading().plans);
        let Some(plans) = &load.value else {
            return load.missing("Could not read the plan limits", shown.asking_limits(), cx);
        };
        let now = data::now();
        let parts = parts::parts(plans, &here(), now);
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .children(stale(load.error.as_ref()))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(22.))
                    .children(parts.iter().enumerate().map(|(i, part)| limits::part(i, part, load.loading, now))),
            )
            .child(ui::dim(shown.asked_on()))
            .into_any_element()
    }

    fn stats_page(&self, cx: &Context<Self>) -> AnyElement {
        let (shown, load) = (self.shown(), &self.reading().stats);
        let Some(stats) = &load.value else {
            return load.missing("Could not read the stats", shown.asking_stats(), cx);
        };
        let now = data::now();
        let days = stats.days(now);
        let page = div().flex().flex_col().gap(px(14.));
        if stats.empty() && stats.merged_pull_requests.is_none() {
            return page
                .children(stale(load.error.as_ref()))
                .child(card().child(ui::note(EMPTY)))
                .child(stats::heatmap(stats, days))
                .children(notes(stats.notes(now)))
                .into_any_element();
        }
        page.children(stale(load.error.as_ref()))
            .child(stats::hero(stats, days))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .gap(px(14.))
                    .child(div().flex_1().min_w(px(380.)).child(stats::table(stats)))
                    .child(stats::heatmap(stats, days)),
            )
            .children(notes(stats.notes(now)))
            .into_any_element()
    }

    fn window_tabs(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        WINDOWS
            .iter()
            .enumerate()
            .map(|(i, (_, label))| {
                ui::segment(("usage-window", i), *label, self.window == i)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.window = i;
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect()
    }

    fn agents_page(&self, cx: &Context<Self>) -> AnyElement {
        let (shown, load) = (self.shown(), &self.reading().stats);
        let Some(stats) = &load.value else {
            return load.missing("Could not read the stats", shown.asking_stats(), cx);
        };
        let key = WINDOWS[self.window.min(WINDOWS.len() - 1)].0;
        let agents = stats.agents(key);
        let count = plural(agents.len() as u64, "agent", "agents");
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(ui::segments(self.window_tabs(cx)))
                    .child(div().flex_1())
                    .child(ui::dim(format!("{count} with work in this window"))),
            )
            .child(if agents.is_empty() {
                card().child(ui::note("No agent worked in this window.")).into_any_element()
            } else {
                agents::by_agent(&agents).into_any_element()
            })
            .children(notes(stats.machine_notes(data::now())))
            .child(ui::dim(format!(
                "cornercase counts per agent only: it keeps no totals per project or model, and none of it leaves {}.",
                shown.counted_on()
            )))
            .into_any_element()
    }

    fn body(&self, cx: &Context<Self>) -> AnyElement {
        match self.page {
            Page::Limits => self.limits_page(cx),
            Page::Stats => self.stats_page(cx),
            Page::Agents => self.agents_page(cx),
        }
    }
}

impl Render for UsagePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(13.))
            .text_color(colours::text())
            .child(header_bar().flex().flex_col().gap(px(10.)).children(self.picker(cx)).child(self.header(cx)))
            .child(div().id("usage-body").flex_1().overflow_y_scroll().px(px(20.)).py(px(16.)).child(self.body(cx)))
    }
}

fn here() -> String {
    cli::host().unwrap_or_else(|| THIS_COMPUTER.to_string())
}

fn header_bar() -> Div {
    div().flex_none().px(px(20.)).py(px(12.)).border_b_1().border_color(colours::border())
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| UsagePanel::new(window, cx)).into()
}

async fn tick_while_alive(this: WeakEntity<UsagePanel>, cx: &mut AsyncApp) {
    loop {
        cx.background_executor().timer(TICK).await;
        if this.update(cx, UsagePanel::tick).is_err() {
            break;
        }
    }
}

fn run_json<T: serde::de::DeserializeOwned + 'static>(
    cx: &mut Context<UsagePanel>,
    args: Vec<String>,
    then: impl FnOnce(&mut UsagePanel, anyhow::Result<T>, &mut Context<UsagePanel>) + 'static,
) {
    store::run_partial(cx, args, move |this, out, cx| {
        then(this, out.and_then(|out| Ok(serde_json::from_str::<T>(&out)?)), cx);
    });
}
