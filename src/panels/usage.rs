mod agents;
mod data;
mod limits;
mod stats;
mod widgets;

use std::process::Stdio;
use std::time::Duration;

use anyhow::bail;
use gpui::{
    AnyElement, AnyView, AppContext as _, AsyncApp, ClickEvent, ClipboardItem, Context, Div, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Task,
    WeakEntity, Window, div, prelude::FluentBuilder as _, px,
};

use self::data::{Plans, Stats, WINDOWS};
use self::widgets::{card, failure, loading, plural, stale};
use crate::cli;
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
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn missing(&self, title: &'static str, waiting: &'static str, cx: &Context<UsagePanel>) -> AnyElement {
        match &self.error {
            Some(e) => failure(title, e, cx).into_any_element(),
            None => loading(waiting).into_any_element(),
        }
    }
}

pub(crate) struct UsagePanel {
    page: Page,
    window: usize,
    stats: Load<Stats>,
    plans: Load<Plans>,
    sharing: bool,
    open: bool,
    _store: Subscription,
    _ticker: Task<()>,
}

impl UsagePanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        let subscription = cx.observe(&store, |this, store, cx| {
            let open = store.read(cx).sheet == Some(PanelKind::Usage);
            if open && !this.open {
                this.refresh(false, cx);
            }
            this.open = open;
        });
        let ticker = cx.spawn(async move |this, cx| tick_while_alive(this, cx).await);
        let mut panel = Self {
            page: Page::Stats,
            window: 0,
            stats: Load::default(),
            plans: Load::default(),
            sharing: false,
            open: store.read(cx).sheet == Some(PanelKind::Usage),
            _store: subscription,
            _ticker: ticker,
        };
        panel.refresh(false, cx);
        panel
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
        if self.stats.loading {
            return;
        }
        self.stats.loading = true;
        cx.notify();
        run_json(cx, store::args(&["stats", "--json"]), |this, result: anyhow::Result<Stats>, cx| {
            this.stats.done(result);
            cx.notify();
        });
    }

    fn load_plans(&mut self, forced: bool, cx: &mut Context<Self>) {
        if self.plans.loading {
            return;
        }
        if !forced && self.plans.value.is_none() {
            run_json(cx, store::args(&["usage", "--cached", "--json"]), Self::cached_plans);
        }
        self.plans.loading = true;
        cx.notify();
        run_json(cx, store::args(&["usage", "--json"]), |this, result: anyhow::Result<Plans>, cx| {
            this.plans.done(result);
            cx.notify();
        });
    }

    fn cached_plans(&mut self, result: anyhow::Result<Plans>, cx: &mut Context<Self>) {
        if self.plans.value.is_none()
            && let Ok(plans) = result
        {
            self.plans.value = Some(plans);
            cx.notify();
        }
    }

    fn switch(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page == page {
            return;
        }
        self.page = page;
        let stale = match page {
            Page::Limits => self.plans.value.is_none() || self.plans.at.is_none_or(|at| data::now() - at > 60),
            Page::Stats | Page::Agents => self.stats.at.is_none_or(|at| data::now() - at > 30),
        };
        if stale {
            self.refresh(false, cx);
        }
        cx.notify();
    }

    fn share(&mut self, cx: &mut Context<Self>) {
        if self.sharing {
            return;
        }
        self.sharing = true;
        cx.notify();
        store::run(cx, store::args(&["stats", "--share"]), |this, result, cx| {
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
        let (loading, at) = match self.page {
            Page::Limits => (self.plans.loading, self.plans.at),
            Page::Stats | Page::Agents => (self.stats.loading, self.stats.at),
        };
        let status = if loading {
            "loading…".to_string()
        } else {
            at.map(|at| data::updated(data::now() - at)).unwrap_or_default()
        };
        (loading, status)
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
            && self.stats.value.as_ref().is_some_and(|s| !s.empty() || s.merged_pull_requests.is_some());
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(ui::segments(tabs))
            .child(div().flex_1())
            .child(ui::dim(status))
            .when(can_share, |d| {
                d.child(
                    ui::button("usage-share", if self.sharing { "Copying…" } else { "Copy card link" })
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.share(cx))),
                )
            })
            .child(
                ui::button("usage-refresh", if loading { "Refreshing…" } else { "Refresh" })
                    .when(loading, |d| d.cursor_default().opacity(0.6))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(true, cx))),
            )
    }

    fn limits_page(&self, cx: &Context<Self>) -> AnyElement {
        let Some(plans) = &self.plans.value else {
            return self.plans.missing(
                "Could not read the plan limits",
                "Asking Claude Code and Codex for their limits…",
                cx,
            );
        };
        let now = data::now();
        let fresh = self.plans.loading;
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .children(stale(self.plans.error.as_ref()))
            .when(plans.agents.is_empty(), |d| d.child(ui::note("No agent with plan limits is installed.")))
            .children(plans.agents.iter().enumerate().map(|(i, plan)| limits::plan_card(i, plan, fresh, now)))
            .child(ui::dim(
                "Asked on the server's machine with no prompt and no tokens. Cursor CLI, Gemini CLI, opencode, \
                 Grok Build and Copilot have no plan limits cornercase can read.",
            ))
            .into_any_element()
    }

    fn stats_page(&self, cx: &Context<Self>) -> AnyElement {
        let Some(stats) = &self.stats.value else {
            return self.stats.missing("Could not read the stats", "Reading the stats…", cx);
        };
        let days = stats.days(data::now());
        let page = div().flex().flex_col().gap(px(14.));
        if stats.empty() && stats.merged_pull_requests.is_none() {
            return page.child(card().child(ui::note(EMPTY))).child(stats::heatmap(stats, days)).into_any_element();
        }
        page.children(stale(self.stats.error.as_ref()))
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
        let Some(stats) = &self.stats.value else {
            return self.stats.missing("Could not read the stats", "Reading the stats…", cx);
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
            .child(ui::dim(
                "cornercase counts per agent only: it keeps no totals per project or model, and none of it leaves \
                 your machine.",
            ))
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
            .child(header_bar().child(self.header(cx)))
            .child(div().id("usage-body").flex_1().overflow_y_scroll().px(px(20.)).py(px(16.)).child(self.body(cx)))
    }
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

fn run_json<T: serde::de::DeserializeOwned + Send + 'static>(
    cx: &mut Context<UsagePanel>,
    args: Vec<String>,
    then: impl FnOnce(&mut UsagePanel, anyhow::Result<T>, &mut Context<UsagePanel>) + 'static,
) {
    let task = cx.background_spawn(async move {
        let out = cli::command(&args).stdin(Stdio::null()).output()?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() && stdout.trim().is_empty() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let name = args.first().map(String::as_str).unwrap_or_default();
            bail!("cornercase {name}: {}", if err.is_empty() { "failed".into() } else { err });
        }
        Ok(serde_json::from_str::<T>(&stdout)?)
    });
    crate::panels::flows::deliver(cx, task, then);
}
