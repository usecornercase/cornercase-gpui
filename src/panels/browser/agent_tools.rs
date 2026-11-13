use std::time::{Duration, Instant};

use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, Corner, Div, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    anchored, deferred, div, point, prelude::FluentBuilder as _, px,
};

use super::Browser;
use super::agent::Hand;
use super::agent::far::{self, Drive};
use super::agent::{Agent, Menu, driving_note, no_agent_there, toast};
use super::shots::Shot;
use crate::cli;
use crate::dialogs;
use crate::dialogs::Question;
use crate::model::tunnels::Toward;
use crate::notify::tip;
use crate::store::{Store, ToastKind, copied_words};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone, glass};

const BUTTON: f32 = 26.;
const REOPEN: Duration = Duration::from_millis(400);

fn popover(card: impl IntoElement, below: bool) -> Div {
    let (spot, corner, lift) = if below {
        (div().absolute().top_full().right_0(), Corner::TopRight, 6.)
    } else {
        (div().absolute().top_0().left_0(), Corner::BottomLeft, -6.)
    };
    let placed = anchored().anchor(corner).offset(point(px(0.), px(lift))).snap_to_window_with_margin(px(8.));
    spot.child(deferred(placed.child(card)).with_priority(1))
}

fn entry(id: impl Into<gpui::ElementId>, glyph: Icon, label: impl Into<SharedString>) -> Stateful<Div> {
    ui::row(id, false)
        .text_size(px(13.))
        .text_color(colours::soft())
        .child(ui::icon(glyph, 14.).text_color(colours::muted()))
        .child(ui::one_line(label))
}

fn menu_card(id: &'static str, width: f32, cx: &mut Context<Browser>) -> Stateful<Div> {
    glass::card(radius::card())
        .id(id)
        .occlude()
        .w(px(width))
        .p(px(4.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| cx.stop_propagation())
        .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_menu(cx)))
}

impl Browser {
    pub(super) fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        let just_closed = self.agents.closed.take().is_some_and(|(m, at)| m == menu && at.elapsed() < REOPEN);
        if !just_closed {
            self.agents.menu = if self.agents.menu == Some(menu) { None } else { Some(menu) };
            cx.notify();
        }
    }

    pub(super) fn drop_chooser(&mut self) {
        if self.agents.menu == Some(Menu::Chooser) {
            self.agents.menu = None;
        }
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = self.agents.menu.take() {
            self.agents.closed = Some((menu, Instant::now()));
            cx.notify();
        }
    }

    pub(super) fn pick_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let enabled = self.active_session().is_some();
        ui::tool_button("browser-pick", Icon::Pick, BUTTON, self.picking())
            .when(!enabled, |b| b.opacity(0.35))
            .tooltip(tip("Pick an element to hand to the agent"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_pick(cx)))
    }

    pub(super) fn shots_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let open = self.agents.menu == Some(Menu::Shots);
        let button = if self.agents.shooting.is_some() {
            div().size(px(BUTTON)).flex().items_center().justify_center().child(ui::spinner(
                "browser-shooting",
                14.,
                colours::accent(),
            ))
        } else {
            div().child(
                ui::tool_button("browser-shot", Icon::Camera, BUTTON, open)
                    .when(self.active_session().is_none(), |b| b.opacity(0.35))
                    .tooltip(tip("Screenshot for the agent"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_menu(Menu::Shots, cx))),
            )
        };
        let menu = (open && self.agents.shooting.is_none()).then(|| popover(shots_menu(cx), true));
        div().relative().flex_none().child(button).children(menu)
    }

    pub(super) fn agents_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let open = self.agents.menu == Some(Menu::Agents);
        let button = ui::tool_button("browser-agents", Icon::Agent, BUTTON, open)
            .tooltip(tip("Agents and this browser"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_menu(Menu::Agents, cx)));
        let menu = open.then(|| popover(self.agents_menu(cx), true));
        div().relative().flex_none().child(button).children(menu)
    }

    fn far_name(&self) -> String {
        self.place.machine().map(str::to_string).or_else(cli::host).unwrap_or_default()
    }

    fn agents_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let drive = self.drive(cx);
        let name = self.far_name();
        let endpoint = self.agent_endpoint(&drive);
        let shows_endpoint = matches!(drive, Drive::Here | Drive::Own | Drive::Open(_));
        let copied = matches!(drive, Drive::Open(_)).then(|| copied_words(&name));
        menu_card("browser-agents-menu", 320., cx)
            .child(ui::section("Agents and this browser").px(px(10.)).pt(px(6.)))
            .child(div().px(px(10.)).text_size(px(12.)).text_color(colours::muted()).child(far::says(&drive, &name)))
            .children(drive_note(&drive, &name))
            .children(self.open_control(&drive, &name, cx))
            .children(shows_endpoint.then(|| endpoint_row(endpoint.clone(), copied, cx).into_any_element()))
            .children(self.tell_row(&drive, endpoint.is_some(), cx))
            .children(self.close_control(&drive, &name, cx))
            .children(self.agent_rows(cx))
            .child(div().mx(px(6.)).my(px(4.)).h(px(1.)).bg(colours::border()))
            .child(self.toggles(cx))
    }

    fn open_control(&self, drive: &Drive, name: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        match drive {
            Drive::Opening => Some(far_busy(far::opening_words(name))),
            Drive::Closed if self.port().is_none() => Some(far_busy("Starting the browser…".to_string())),
            Drive::Closed => {
                let toward = self.place.toward(cli::host().as_deref())?;
                let name = name.to_string();
                let label = far::open_label(&name);
                let row = entry("browser-agents-open-far", Icon::Share, label).tooltip(tip(far::open_tooltip(&name)));
                Some(
                    row.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.agents.menu = None;
                        Self::ask_to_open(toward.clone(), &name, window, cx);
                        cx.notify();
                    }))
                    .into_any_element(),
                )
            }
            _ => None,
        }
    }

    fn close_control(&self, drive: &Drive, name: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Drive::Open(_) = drive else { return None };
        let toward = self.place.toward(cli::host().as_deref())?;
        let row = entry("browser-agents-close-far", Icon::Close, far::close_label(name));
        Some(
            row.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.agents.menu = None;
                Store::get(cx).update(cx, |s, cx| s.close_reverse(&toward, cx));
                cx.notify();
            }))
            .into_any_element(),
        )
    }

    fn ask_to_open(toward: Toward, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (title, message, button) = far::confirm_words(name);
        let panel = cx.entity();
        let question = Question::new(title, message, button, Vec::new()).on_yes(move |cx| {
            let chrome = panel.read(cx).port();
            Store::get(cx).update(cx, |s, cx| match chrome {
                Some(chrome) => s.open_reverse(&toward, chrome, cx),
                None => s.not_opened("This Chrome is not running", cx),
            });
        });
        dialogs::confirm(question, window, cx);
    }

    fn no_agent_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let row = div().flex().items_center().gap(px(8.)).p(px(6.));
        if let Some(name) = self.place.machine() {
            return row.child(div().flex_1().child(ui::note(no_agent_there(name)))).into_any_element();
        }
        row.child(div().flex_1().child(ui::note("No agent in this workspace")))
            .child(ui::icon_text_button("browser-agents-start", Icon::Plus, "Start an agent", Tone::Plain).on_click(
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.agents.menu = None;
                    dialogs::start_agent(window, cx);
                }),
            ))
            .into_any_element()
    }

    fn tell_row(&self, drive: &Drive, ready: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(agent) = self.target_agent(cx) else { return Some(self.no_agent_row(cx)) };
        if !far::offers_tell(drive) {
            return None;
        }
        let label = format!("Tell {} about it", Self::agent_name_on(&agent, cx));
        entry("browser-agents-tell", Icon::Send, label)
            .when(!ready, |e| e.opacity(0.35))
            .tooltip(tip("Pastes a note into its prompt: where this Chrome runs and how to connect to it"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.tell_agent(cx)))
            .into_any_element()
            .into()
    }

    fn agent_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let choices = self.agent_choices(cx);
        if choices.len() < 2 {
            return Vec::new();
        }
        let target = self.target_agent(cx);
        let rows = choices.into_iter().map(|choice| {
            let chosen = target.as_ref().map(Agent::key) == Some(choice.pane);
            let glyph = if chosen { Icon::Check } else { Icon::Agent };
            entry(SharedString::from(format!("browser-agent-{}", choice.pane.element())), glyph, choice.label)
                .when(chosen, |e| e.text_color(colours::accent()))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_agent(choice.pane, cx)))
                .into_any_element()
        });
        std::iter::once(ui::section("Hand things to").px(px(10.)).pt(px(6.)).into_any_element()).chain(rows).collect()
    }

    pub(super) fn agent_menu_up(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let rows = self.agent_rows(cx);
        popover(menu_card("browser-agent-chooser", 280., cx).children(rows), false)
    }

    fn toggles(&self, cx: &mut Context<Self>) -> Div {
        let reload =
            ui::checkbox("browser-agents-reload", !self.agents.watch.off, "Reload pages when an agent finishes")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.agents.watch.off = !this.agents.watch.off;
                    cx.notify();
                }));
        let follow = ui::checkbox("browser-agents-follow", !self.agents.follow.off, "Show the tabs agents open")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.agents.follow.off = !this.agents.follow.off;
                cx.notify();
            }));
        div().flex().flex_col().gap(px(6.)).px(px(10.)).py(px(6.)).child(reload).child(follow)
    }

    fn tell_agent(&mut self, cx: &mut Context<Self>) {
        let Some(agent) = self.target_agent(cx) else { return };
        let Some(url) = self.agent_endpoint(&self.drive(cx)) else { return };
        self.agents.menu = None;
        let said = format!("Told {} where this Chrome runs", Self::agent_name_on(&agent, cx));
        Self::hand(&agent, Hand { text: driving_note(&url), files: Vec::new(), submit: false }, said, cx);
        cx.notify();
    }
}

fn shots_menu(cx: &mut Context<Browser>) -> impl IntoElement + use<> {
    menu_card("browser-shots-menu", 220., cx)
        .child(ui::section("Screenshot for the agent").px(px(10.)).pt(px(6.)).pb(px(2.)))
        .child(
            entry("browser-shot-visible", Icon::Monitor, "Visible area")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.shoot(Shot::Visible, cx))),
        )
        .child(
            entry("browser-shot-full", Icon::File, "Full page")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.shoot(Shot::Full, cx))),
        )
}

fn drive_note(drive: &Drive, name: &str) -> Option<AnyElement> {
    match drive {
        Drive::Needs(why) => {
            Some(div().px(px(10.)).pt(px(2.)).text_size(px(12.)).child(ui::dim(why.clone())).into_any_element())
        }
        Drive::Own => Some(ui::warning_line(far::own_exposure(name)).mx(px(10.)).mt(px(4.)).into_any_element()),
        Drive::Open(_) => Some(ui::warning_line(far::exposure(name)).mx(px(10.)).mt(px(4.)).into_any_element()),
        Drive::Here | Drive::Closed | Drive::Opening => None,
    }
}

fn far_busy(text: String) -> AnyElement {
    div().px(px(10.)).py(px(6.)).child(ui::busy("browser-agents-far", text)).into_any_element()
}

fn endpoint_row(
    endpoint: Option<String>,
    copied_words: Option<String>,
    cx: &mut Context<Browser>,
) -> impl IntoElement + use<> {
    let Some(url) = endpoint else {
        return div().px(px(10.)).py(px(6.)).child(ui::busy("browser-agents-endpoint", "Starting the browser…"));
    };
    let copied = url.clone();
    div()
        .mx(px(6.))
        .my(px(4.))
        .pl(px(8.))
        .pr(px(4.))
        .py(px(4.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(radius::control()))
        .bg(colours::panel())
        .child(div().flex_1().min_w_0().flex().text_size(px(12.)).text_color(colours::text()).child(ui::one_line(url)))
        .child(ui::tool_button("browser-agents-copy", Icon::Copy, 22., false).tooltip(tip("Copy")).on_click(
            cx.listener(move |_, _: &ClickEvent, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()));
                let words = copied_words.clone().unwrap_or_else(|| "Copied the browser's DevTools address".to_string());
                toast(words, ToastKind::Info, cx);
            }),
        ))
}

pub(super) fn reloaded(label: String) -> impl IntoElement {
    let pill = glass::card(radius::card())
        .flex()
        .items_center()
        .gap(px(6.))
        .max_w(px(260.))
        .px(px(10.))
        .py(px(5.))
        .text_size(px(12.))
        .text_color(colours::soft())
        .child(ui::icon(Icon::Refresh, 12.).text_color(colours::accent()))
        .child(ui::one_line(label));
    div().absolute().top(px(12.)).right(px(12.)).child(pill)
}
