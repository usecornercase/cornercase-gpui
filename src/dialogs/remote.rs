mod hosts;

use std::rc::Rc;

use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, ElementId, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window, div, prelude::FluentBuilder as _, px,
};

use super::{cancel_and, close, frame, show, text_field};
use crate::cli::{self, Remote, Target};
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, Icon, TextField};

const HOW: &str = "Every command runs as ssh DESTINATION cornercase …, with your ssh config, keys and agent, and \
                   the server starts there if none runs. It never asks for a password: use a key or an agent.";
const WHERE: &str = "The shells, agents and files stay on that machine. cornercase is looked for on its PATH, in \
                     ~/.local/bin, ~/.cargo/bin and Homebrew's folders.";

pub(crate) struct Connect {
    destination: Entity<TextField>,
    program: Entity<TextField>,
    hosts: Vec<String>,
    current: Target,
}

pub(crate) fn connect(window: &mut Window, cx: &mut App) {
    let current = cli::target();
    let view = cx.new(|cx| {
        let submit = |this: &mut Connect, _: &mut Window, cx: &mut Context<Connect>| this.submit(cx);
        let destination = text_field("a host from ~/.ssh/config, or user@host", cx, submit, window);
        let program = text_field("cornercase", cx, submit, window);
        if let Target::Remote(remote) = &current {
            destination.update(cx, |f, cx| f.set_text(remote.destination().to_string(), cx));
            program.update(cx, |f, cx| f.set_text(remote.program().unwrap_or_default().to_string(), cx));
        }
        let mut hosts: Vec<String> = current.host().map(str::to_string).into_iter().collect();
        hosts.extend(hosts::read().into_iter().filter(|host| current.host() != Some(host)));
        Connect { destination, program, hosts, current }
    });
    let focus = view.read(cx).destination.read(cx).focus_handle(cx);
    show(view.into(), &focus, window, cx);
}

fn place_row(id: impl Into<ElementId>, glyph: Icon, label: impl Into<SharedString>, current: bool) -> Stateful<Div> {
    ui::row(id, current)
        .child(ui::icon(glyph, 15.).text_color(if current { colours::accent() } else { colours::muted() }))
        .child(div().flex_1().min_w_0().child(ui::one_line(label.into())))
        .when(current, |d| d.child(ui::dim("current")))
}

impl Connect {
    fn submit(&mut self, cx: &mut Context<Self>) {
        let destination = self.destination.read(cx).text();
        let program = self.program.read(cx).text();
        if let Some(remote) = Remote::new(&destination, &program) {
            Store::get(cx).update(cx, |s, cx| s.connect(Target::Remote(remote), cx));
        }
    }

    fn pick(&mut self, host: &str, clicks: usize, cx: &mut Context<Self>) {
        let program = match &self.current {
            Target::Remote(remote) if remote.destination() == host => remote.program().unwrap_or_default(),
            _ => "",
        }
        .to_string();
        self.destination.update(cx, |f, cx| f.set_text(host.to_string(), cx));
        self.program.update(cx, |f, cx| f.set_text(program, cx));
        if clicks > 1 {
            self.submit(cx);
        }
    }

    fn places(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let here = place_row("connect-here", Icon::Laptop, "This Mac", self.current == Target::Local)
            .on_click(|_: &ClickEvent, _, cx| Store::get(cx).update(cx, |s, cx| s.connect(Target::Local, cx)));
        let current = self.current.host().map(str::to_string);
        let hosts = self.hosts.iter().enumerate().map(|(i, host)| {
            let to = host.clone();
            place_row(("connect-host", i), Icon::Server, host.clone(), current.as_deref() == Some(host))
                .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| this.pick(&to, e.click_count(), cx)))
        });
        div()
            .id("connect-places")
            .max_h(px(216.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(here)
            .children(hosts.collect::<Vec<_>>())
    }
}

impl Render for Connect {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ready = Remote::new(&self.destination.read(cx).text(), "").is_some();
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.places(cx))
            .child(ui::field("Destination", ui::boxed(&self.destination, window, cx)))
            .child(ui::field("Run cornercase there with (optional)", ui::boxed(&self.program, window, cx)))
            .child(div().flex().flex_col().gap(px(4.)).child(ui::dim(HOW)).child(ui::dim(WHERE)));
        let button = ui::primary_button("connect", "Connect")
            .when(!ready, |b| b.opacity(0.5))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx)));
        frame("Connect to a server", body, cancel_and(button))
    }
}

pub(crate) struct PathQuestion {
    pub title: String,
    pub button: &'static str,
}

type Then = Rc<dyn Fn(String, &mut Window, &mut App)>;

pub(crate) struct AskPath {
    question: PathQuestion,
    host: String,
    field: Entity<TextField>,
    then: Then,
}

pub(crate) fn ask_path(
    question: PathQuestion,
    then: impl Fn(String, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let host = cli::host().unwrap_or_default();
    let view = cx.new(|cx| {
        let field =
            text_field("~/src/my-project", cx, |this: &mut AskPath, window, cx| this.submit(window, cx), window);
        AskPath { question, host, field, then: Rc::new(then) }
    });
    let focus = view.read(cx).field.read(cx).focus_handle(cx);
    show(view.into(), &focus, window, cx);
}

impl AskPath {
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.field.read(cx).text();
        if typed.trim().is_empty() {
            return;
        }
        close(cx);
        (self.then)(cli::from_home(&typed), window, cx);
    }
}

impl Render for AskPath {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(ui::field(format!("Folder on {}", self.host), ui::boxed(&self.field, window, cx)))
            .child(ui::dim("From its home folder, or from /. The folder picker only sees this Mac's disk."));
        let button = ui::primary_button("ask-path", self.question.button)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.submit(window, cx)));
        frame(self.question.title.clone(), body, cancel_and(button))
    }
}
