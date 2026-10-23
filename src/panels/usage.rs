//! The Usage sheet: what agents did in cornercase (`cornercase stats --json`: how much they worked
//! in parallel, the counters per window, a heatmap of the last 26 weeks, the totals per agent and
//! the shareable card link from `stats --share`) and how much of each plan is left
//! (`cornercase usage --json`: Claude Code's and Codex's limits with their reset times).
//! Like the TUI's Usage dialog it opens on Stats, and asks the agents for their limits only when
//! the Limits tab shows, since each probe runs `claude`/`codex` for a few seconds.

mod data;

use std::process::Stdio;
use std::time::Duration;

use anyhow::bail;
use gpui::{
    AnyElement, AnyView, AppContext as _, ClickEvent, ClipboardItem, Context, Div, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Rgba, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div, prelude::FluentBuilder as _, px,
    relative,
};

use self::data::{Counts, Limit, Plan, Plans, Severity, Stats, WEEKS, WINDOWS};
use crate::cli;
use crate::panels::PanelKind;
use crate::store::{self, Store, ToastKind};
use crate::theme::{self, colours};
use crate::ui;

/// What the TUI says before anything was counted.
const EMPTY: &str = "nothing counted yet: agents running in cornercase count from now on";
/// The issue the Codex probe's failure on older Codex versions waits on.
const CODEX_ISSUE: &str = "usecornercase/cornercase-terminal#462";

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

/// One command's answer as the sheet keeps it: the last value, whether a new one is coming, and
/// the last failure.
struct Load<T> {
    value: Option<T>,
    loading: bool,
    error: Option<String>,
    /// When the value arrived, in Unix seconds.
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
}

pub(crate) struct UsagePanel {
    page: Page,
    /// The window the By agent page shows, an index in `WINDOWS`.
    window: usize,
    stats: Load<Stats>,
    plans: Load<Plans>,
    /// `stats --share` is running.
    sharing: bool,
    /// The sheet was open at the last store change, to refresh when it opens again.
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
        // Ages and reset times are relative: draw them again now and then while the sheet shows.
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(20)).await;
                let alive = this.update(cx, |this, cx| {
                    if this.open {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
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

    // -----------------------------------------------------------------------------------------
    // Loading.

    /// Reads what the page shows again. The stats are cheap; the limits run the probes, so they
    /// are asked only on the Limits page (cached first, so the last answers show at once).
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
            run_json(cx, store::args(&["usage", "--cached", "--json"]), |this, result: anyhow::Result<Plans>, cx| {
                // The fresh answer may have come first; it wins.
                if this.plans.value.is_none()
                    && let Ok(plans) = result
                {
                    this.plans.value = Some(plans);
                    cx.notify();
                }
            });
        }
        self.plans.loading = true;
        cx.notify();
        run_json(cx, store::args(&["usage", "--json"]), |this, result: anyhow::Result<Plans>, cx| {
            this.plans.done(result);
            cx.notify();
        });
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

    /// Copies the card link (`stats --share`, made locally: nothing is uploaded).
    fn share(&mut self, cx: &mut Context<Self>) {
        if self.sharing {
            return;
        }
        self.sharing = true;
        cx.notify();
        store::run(cx, store::args(&["stats", "--share"]), |this, result, cx| {
            this.sharing = false;
            let store = Store::get(cx);
            match result.map(|out| out.trim().to_string()) {
                Ok(link) if !link.is_empty() => {
                    cx.write_to_clipboard(ClipboardItem::new_string(link));
                    store.update(cx, |s, cx| s.toast("copied a link to your stats card", ToastKind::Done, cx));
                }
                Ok(_) => store.update(cx, |s, cx| s.toast("cornercase gave no stats link", ToastKind::Error, cx)),
                Err(e) => store.update(cx, |s, cx| s.toast(e.to_string(), ToastKind::Error, cx)),
            }
            cx.notify();
        });
    }

    // -----------------------------------------------------------------------------------------
    // Drawing.

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tabs = Page::ALL
            .into_iter()
            .map(|(page, label)| {
                ui::segment(("usage-page", page as usize), label, self.page == page)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.switch(page, cx)))
                    .into_any_element()
            })
            .collect();
        let (loading, at) = match self.page {
            Page::Limits => (self.plans.loading, self.plans.at),
            Page::Stats | Page::Agents => (self.stats.loading, self.stats.at),
        };
        let status = if loading {
            "loading…".to_string()
        } else {
            at.map(|at| data::updated(data::now() - at)).unwrap_or_default()
        };
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

    fn render_limits(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(plans) = &self.plans.value else {
            return match &self.plans.error {
                Some(e) => failure("Could not read the plan limits", e, cx).into_any_element(),
                None => loading("Asking Claude Code and Codex for their limits…").into_any_element(),
            };
        };
        let now = data::now();
        let fresh = self.plans.loading;
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .when_some(self.plans.error.as_ref(), |d, e| d.child(error_line(format!("could not refresh: {e}"))))
            .when(plans.agents.is_empty(), |d| d.child(ui::note("No agent with plan limits is installed.")))
            .children(plans.agents.iter().enumerate().map(|(i, plan)| plan_card(i, plan, fresh, now)))
            .child(ui::dim(
                "Asked on the server's machine with no prompt and no tokens. Cursor CLI, Gemini CLI, opencode, \
                 Grok Build and Copilot have no plan limits cornercase can read.",
            ))
            .into_any_element()
    }

    fn render_stats(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(stats) = &self.stats.value else {
            return match &self.stats.error {
                Some(e) => failure("Could not read the stats", e, cx).into_any_element(),
                None => loading("Reading the stats…").into_any_element(),
            };
        };
        let now = data::now();
        let days = stats.days(now);
        if stats.empty() && stats.merged_pull_requests.is_none() {
            return div()
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(card().child(ui::note(EMPTY)))
                .child(heatmap(stats, days))
                .into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .when_some(self.stats.error.as_ref(), |d, e| d.child(error_line(format!("could not refresh: {e}"))))
            .child(hero(stats, days))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .gap(px(14.))
                    .child(div().flex_1().min_w(px(380.)).child(table(stats)))
                    .child(heatmap(stats, days)),
            )
            .into_any_element()
    }

    fn render_agents(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(stats) = &self.stats.value else {
            return match &self.stats.error {
                Some(e) => failure("Could not read the stats", e, cx).into_any_element(),
                None => loading("Reading the stats…").into_any_element(),
            };
        };
        let windows = WINDOWS
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
            .collect();
        let key = WINDOWS[self.window.min(WINDOWS.len() - 1)].0;
        let agents = stats.agents(key);
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div().flex().items_center().gap(px(10.)).child(ui::segments(windows)).child(div().flex_1()).child(
                    ui::dim(format!("{} with work in this window", plural(agents.len() as u64, "agent", "agents"))),
                ),
            )
            .child(if agents.is_empty() {
                card().child(ui::note("No agent worked in this window.")).into_any_element()
            } else {
                by_agent(&agents).into_any_element()
            })
            .child(ui::dim(
                "cornercase counts per agent only: it keeps no totals per project or model, and none of it leaves \
                 your machine.",
            ))
            .into_any_element()
    }
}

impl Render for UsagePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.page {
            Page::Limits => self.render_limits(cx),
            Page::Stats => self.render_stats(cx),
            Page::Agents => self.render_agents(cx),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(13.))
            .text_color(colours::text())
            .child(
                div()
                    .flex_none()
                    .px(px(20.))
                    .py(px(12.))
                    .border_b_1()
                    .border_color(colours::border())
                    .child(self.render_header(cx)),
            )
            .child(div().id("usage-body").flex_1().overflow_y_scroll().px(px(20.)).py(px(16.)).child(body))
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| UsagePanel::new(window, cx)).into()
}

// ---------------------------------------------------------------------------------------------
// Running commands.

/// Runs a command off the main thread and parses its JSON. `cornercase usage` exits 1 when an
/// agent could not be asked, after printing everything, so what it printed counts even then.
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
    cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| then(this, result, cx));
    })
    .detach();
}

// ---------------------------------------------------------------------------------------------
// Pieces.

fn card() -> Div {
    div().p(px(16.)).rounded(px(8.)).bg(colours::terminal()).border_1().border_color(colours::border())
}

fn heading(text: impl Into<SharedString>) -> Div {
    div().text_size(px(11.)).text_color(colours::faint()).child(text.into().to_uppercase())
}

fn error_line(text: impl Into<SharedString>) -> Div {
    div().text_size(px(12.)).text_color(colours::failed()).child(text.into())
}

fn loading(text: &'static str) -> Div {
    card().child(div().flex().items_center().gap(px(8.)).text_color(colours::muted()).child("◐").child(text))
}

fn failure(title: &'static str, error: &str, cx: &mut Context<UsagePanel>) -> Div {
    card()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(div().text_color(colours::failed()).font_weight(FontWeight::SEMIBOLD).child(title))
        .child(div().text_size(px(12.)).text_color(colours::muted()).child(error.to_string()))
        .child(
            div().child(
                ui::button("usage-retry", "Try again")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(true, cx))),
            ),
        )
}

/// A thin rounded bar: `fraction` of the track filled with `colour`.
fn bar(fraction: f32, colour: impl Into<Hsla>, height: f32) -> Div {
    let fraction = fraction.clamp(0., 1.);
    div()
        .w_full()
        .h(px(height))
        .rounded(px(height / 2.))
        .bg(colours::border())
        .overflow_hidden()
        .when(fraction > 0., |d| {
            d.child(div().h_full().min_w(px(height)).w(relative(fraction)).rounded(px(height / 2.)).bg(colour.into()))
        })
}

fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

// ---------------------------------------------------------------------------------------------
// Limits.

fn severity_colour(severity: Severity) -> Rgba {
    match severity {
        Severity::Normal => colours::done(),
        Severity::Warning => colours::waiting(),
        Severity::Critical => colours::failed(),
    }
}

fn plan_card(i: usize, plan: &Plan, fresh: bool, now: i64) -> impl IntoElement + use<> {
    let status = if fresh { "loading…".to_string() } else { plan.status(now) };
    let codex_bug = plan.agent == "codex" && plan.error.as_deref().is_some_and(|e| e.contains("expected unit"));
    card()
        .id(("usage-plan", i))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(plan.heading()))
                .child(div().flex_1())
                .child(ui::dim(status)),
        )
        .when_some(plan.error.as_ref(), |d, e| d.child(error_line(format!("usage unavailable: {e}"))))
        .when(codex_bug, |d| {
            d.child(ui::dim(format!("a cornercase bug with this Codex version · needs {CODEX_ISSUE}")))
        })
        .when(!plan.limits.is_empty(), |d| {
            d.child(div().flex().flex_col().gap(px(14.)).children(plan.limits.iter().map(|l| limit_row(l, now))))
        })
        .when_some(plan.notice.as_ref(), |d, n| d.child(div().text_color(colours::muted()).child(n.clone())))
        .when(plan.limited == Some(false) && plan.notice.is_none(), |d| {
            d.child(div().text_color(colours::muted()).child("no plan limits"))
        })
        .when_some(plan.extra.as_ref(), |d, e| d.child(ui::dim(e.clone())))
        .when(plan.unasked() && !fresh, |d| d.child(ui::dim("not asked yet")))
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
        .child(bar(limit.fraction(), colour, 6.))
}

// ---------------------------------------------------------------------------------------------
// Stats.

/// How much the agents worked in parallel in the last day: the factor, and agent time against
/// the time any agent was busy.
fn hero(stats: &Stats, days: u64) -> impl IntoElement + use<> {
    let most = stats.window("24h").most_at_once;
    let side = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .w(px(180.))
        .flex_none()
        .child(stat_pill("most at once", most.to_string()))
        .child(stat_pill("counting for", plural(days, "day", "days")));
    let Some((factor, agent, busy)) = stats.parallel() else {
        return card()
            .flex()
            .items_center()
            .gap(px(20.))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div().text_size(px(18.)).text_color(colours::muted()).child("No agent worked in the last 24 h"),
                    )
                    .child(ui::dim("the parallel factor shows once agents have been busy for a minute")),
            )
            .child(side);
    };
    let busy_share = if agent > 0. { (busy / agent) as f32 } else { 0. };
    card()
        .flex()
        .items_center()
        .gap(px(24.))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .child(
                    div()
                        .text_size(px(40.))
                        .line_height(px(44.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(colours::accent())
                        .child(format!("{factor:.1}×")),
                )
                .child(div().text_color(colours::muted()).child("in parallel · last 24 h")),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(time_bar("agent work", data::duration(agent), 1., colours::accent()))
                .child(time_bar("wall time busy", data::duration(busy), busy_share, colours::muted())),
        )
        .child(side)
}

fn time_bar(label: &'static str, value: String, fraction: f32, colour: Rgba) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .flex()
                .text_size(px(12.))
                .child(div().text_color(colours::muted()).child(label))
                .child(div().flex_1())
                .child(value),
        )
        .child(bar(fraction, colour, 8.))
}

fn stat_pill(label: &'static str, value: String) -> Div {
    div()
        .flex()
        .items_center()
        .px(px(10.))
        .py(px(5.))
        .rounded(px(6.))
        .bg(colours::panel())
        .text_size(px(12.))
        .child(div().text_color(colours::muted()).child(label))
        .child(div().flex_1())
        .child(div().font_weight(FontWeight::SEMIBOLD).child(value))
}

/// The TUI's table: nine counters (ten with merged pull requests) in three windows.
fn table(stats: &Stats) -> impl IntoElement + use<> {
    let column =
        |text: SharedString, colour: Rgba| div().w(px(78.)).flex().justify_end().text_color(colour).child(text);
    let header = div()
        .flex()
        .px(px(8.))
        .pb(px(6.))
        .child(div().flex_1())
        .children(WINDOWS.iter().map(|(_, label)| column((*label).into(), colours::faint()).text_size(px(11.))));
    let rows = stats.rows().into_iter().enumerate().map(|(i, (label, values))| {
        div()
            .id(("usage-row", i))
            .flex()
            .px(px(8.))
            .py(px(5.))
            .rounded(px(4.))
            .when(i % 2 == 1, |d| d.bg(colours::panel()))
            .hover(|s| s.bg(colours::hover()))
            .child(div().flex_1().text_color(colours::muted()).child(label))
            .children(values.into_iter().map(|v| column(v.into(), colours::text())))
    });
    let merged_errors: Vec<String> = stats
        .merged_pull_requests
        .iter()
        .flat_map(|m| &m.accounts)
        .filter_map(|a| {
            let who = a.login.as_deref().map_or_else(|| a.host.clone(), |login| format!("{login} on {}", a.host));
            a.error.as_ref().map(|e| format!("{} ({who}): {e}", a.provider))
        })
        .collect();
    card().flex().flex_col().child(header).children(rows).when(!merged_errors.is_empty(), |d| {
        d.child(
            div()
                .pt(px(8.))
                .px(px(8.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(ui::dim("merged PRs marked + are a lower bound, some accounts could not be asked:"))
                .children(merged_errors.into_iter().map(ui::dim)),
        )
    })
}

/// A heatmap level's colour: the line colour for none, then the TUI's blues, darkest first.
fn level_colour(level: u8) -> Hsla {
    match level {
        0 => colours::border().into(),
        1 => theme::hsla(theme::palette(24)),
        2 => theme::hsla(theme::palette(26)),
        3 => theme::hsla(theme::palette(33)),
        _ => theme::hsla(theme::palette(75)),
    }
}

/// The last 26 weeks, one block per rolling 24 hours, seven to a column, newest bottom right.
fn heatmap(stats: &Stats, days: u64) -> impl IntoElement + use<> {
    const CELL: f32 = 13.;
    const GAP: f32 = 3.;
    let levels = stats.levels();
    let level = |i: usize| levels.get(i).copied().unwrap_or(0);
    let columns =
        (0..WEEKS).map(|c| {
            let week = WEEKS - 1 - c;
            div().flex().flex_col().gap(px(GAP)).children(
                (0..7).map(|r| div().size(px(CELL)).rounded(px(3.)).bg(level_colour(level(week * 7 + 6 - r)))),
            )
        });
    let legend = div()
        .flex()
        .items_center()
        .gap(px(3.))
        .text_size(px(11.))
        .text_color(colours::faint())
        .child(div().pr(px(3.)).child("less"))
        .children((1..=4).map(|l| div().size(px(10.)).rounded(px(2.)).bg(level_colour(l))))
        .child(div().pl(px(3.)).child("more"));
    card()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div().flex().items_center().child(heading("agent time, last 26 weeks")).child(div().flex_1()).child(legend),
        )
        .child(div().flex().gap(px(GAP)).children(columns))
        .child(ui::dim(format!("one block per 24 h, the newest bottom right · {}", data::counting(days))))
}

// ---------------------------------------------------------------------------------------------
// By agent.

/// Each agent's colour, from the terminal palette.
fn agent_colour(key: &str) -> Hsla {
    match key {
        "claude" => colours::waiting().into(),
        "codex" => theme::hsla(theme::ANSI[6]),
        "gemini" => theme::hsla(theme::ANSI[4]),
        "opencode" => theme::hsla(theme::ANSI[2]),
        "grok" => theme::hsla(theme::ANSI[7]),
        "cursor" => theme::hsla(theme::ANSI[5]),
        "copilot" => theme::hsla(theme::ANSI[1]),
        _ => colours::muted().into(),
    }
}

fn by_agent(agents: &[(String, Counts)]) -> impl IntoElement + use<> {
    let total: f64 = agents.iter().map(|(_, c)| c.agent_seconds).sum();
    let most = agents.iter().map(|(_, c)| c.agent_seconds).fold(0., f64::max);
    // The share of agent time, one segment per agent.
    let share = div().flex().w_full().h(px(10.)).rounded(px(5.)).overflow_hidden().bg(colours::border()).children(
        agents.iter().filter(|(_, c)| c.agent_seconds > 0.).map(|(key, c)| {
            div().h_full().w(relative((c.agent_seconds / total.max(1e-9)) as f32)).bg(agent_colour(key))
        }),
    );
    let legend = div().flex().flex_wrap().gap(px(14.)).children(agents.iter().map(|(key, c)| {
        let percent = if total > 0. { c.agent_seconds / total * 100. } else { 0. };
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(12.))
            .child(div().size(px(8.)).rounded(px(4.)).bg(agent_colour(key)))
            .child(data::agent_title(key))
            .child(div().text_color(colours::muted()).child(format!("{percent:.0}%")))
    }));
    const COLUMNS: [&str; 7] = ["agent time", "started", "prompts", "turns", "waited", "tokens out", "tokens in"];
    let column = |text: SharedString| div().w(px(72.)).flex_none().flex().justify_end().child(text);
    let header = div()
        .flex()
        .items_center()
        .px(px(8.))
        .pb(px(6.))
        .text_size(px(11.))
        .text_color(colours::faint())
        .child(div().flex_1().child("agent"))
        .children(COLUMNS.iter().map(|c| column((*c).into())));
    let rows = agents.iter().enumerate().map(|(i, (key, c))| {
        let values = [
            data::duration(c.agent_seconds),
            data::count(c.agents),
            data::count(c.prompts),
            data::count(c.turns),
            data::duration(c.waited_seconds),
            data::tokens(c.tokens.out),
            data::tokens(c.tokens.sent()),
        ];
        let fraction = if most > 0. { (c.agent_seconds / most) as f32 } else { 0. };
        div()
            .id(("usage-agent", i))
            .flex()
            .items_center()
            .px(px(8.))
            .py(px(8.))
            .rounded(px(4.))
            .hover(|s| s.bg(colours::hover()))
            .child(
                div()
                    .flex_1()
                    .min_w(px(140.))
                    .pr(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(div().size(px(8.)).rounded(px(4.)).bg(agent_colour(key)))
                            .child(data::agent_title(key))
                            .child(ui::dim(format!("{} from cache", data::tokens(c.tokens.cache_read)))),
                    )
                    .child(bar(fraction, agent_colour(key), 4.)),
            )
            .children(values.into_iter().map(|v| column(v.into())))
    });
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(card().flex().flex_col().gap(px(10.)).child(heading("share of agent time")).child(share).child(legend))
        .child(card().flex().flex_col().child(header).children(rows))
}
