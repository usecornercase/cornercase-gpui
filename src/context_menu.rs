use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Corner, Div, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Pixels, Point, Rgba, SharedString, Stateful, StatefulInteractiveElement,
    Styled, Window, anchored, deferred, div, point, prelude::FluentBuilder as _, px,
};

use crate::app::Root;
use crate::keymap;
use crate::platform::Os;
use crate::store::Busy;
use crate::theme::{colours, radius};
use crate::ui::motion::{self, Way};
use crate::ui::{self, Icon, glass, icon};

const MENU_DROP: f32 = 4.;
const ABOVE_POINTER: f32 = 6.;

pub(crate) type Action = Rc<dyn Fn(&mut Root, &mut Window, &mut Context<Root>)>;

#[derive(Clone)]
pub(crate) enum Entry {
    Item {
        label: SharedString,
        action: Action,
        danger: bool,
        chosen: bool,
        icon: Option<Icon>,
        keys: Option<SharedString>,
    },
    Submenu {
        label: SharedString,
        entries: Vec<Entry>,
    },
    Heading(SharedString),
    Resting {
        label: SharedString,
    },
    Note(SharedString),
    Separator,
}
fn plain(
    label: impl Into<SharedString>,
    action: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
    (danger, chosen): (bool, bool),
) -> Entry {
    Entry::Item { label: label.into(), action: Rc::new(action), danger, chosen, icon: None, keys: None }
}
pub(crate) fn item(
    label: impl Into<SharedString>,
    action: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
) -> Entry {
    plain(label, action, (false, false))
}
pub(crate) fn danger(
    label: impl Into<SharedString>,
    action: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
) -> Entry {
    plain(label, action, (true, false))
}
pub(crate) fn choice(
    label: impl Into<SharedString>,
    chosen: bool,
    action: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
) -> Entry {
    plain(label, action, (false, chosen))
}
pub(crate) fn item_with_keys(
    icon: Icon,
    label: impl Into<SharedString>,
    keys: Option<String>,
    action: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
) -> Entry {
    let Entry::Item { label, action, danger, chosen, .. } = plain(label, action, (false, false)) else {
        unreachable!("plain makes an item")
    };
    Entry::Item { label, action, danger, chosen, icon: Some(icon), keys: keys.map(SharedString::from) }
}
pub(crate) fn act(label: impl Into<SharedString>, args: Vec<String>) -> Entry {
    item(label, move |root, _, _| root.act(args.clone()))
}

pub(crate) fn act_busy(label: impl Into<SharedString>, busy: Busy, args: Vec<String>) -> Entry {
    item(label, move |root, _, cx| {
        let (busy, args) = (busy.clone(), args.clone());
        root.store.update(cx, |s, cx| s.act_busy(busy, args, cx));
    })
}

pub(crate) fn submenu(label: impl Into<SharedString>, entries: Vec<Entry>) -> Entry {
    Entry::Submenu { label: label.into(), entries }
}

pub(crate) fn resting(label: impl Into<SharedString>) -> Entry {
    Entry::Resting { label: label.into() }
}

pub(crate) fn note(text: impl Into<SharedString>) -> Entry {
    Entry::Note(text.into())
}

pub(crate) struct Menu {
    pub at: Point<Pixels>,
    pub entries: Vec<Entry>,
    pub focus: FocusHandle,
    pub highlight: Option<usize>,
    pub from_bottom: bool,
    pub opener: Option<&'static str>,
}

fn choosable(entry: &Entry) -> bool {
    matches!(entry, Entry::Item { .. } | Entry::Submenu { .. })
}

fn step(entries: &[Entry], from: Option<usize>, down: bool) -> Option<usize> {
    let choices: Vec<usize> = entries.iter().enumerate().filter(|(_, e)| choosable(e)).map(|(i, _)| i).collect();
    let at = from.and_then(|from| choices.iter().position(|&i| i == from));
    let next = match at {
        None if down => 0,
        None => choices.len().checked_sub(1)?,
        Some(at) if down => (at + 1) % choices.len(),
        Some(at) => at.checked_sub(1).unwrap_or(choices.len() - 1),
    };
    choices.get(next).copied()
}

fn above(at: Point<Pixels>) -> Point<Pixels> {
    point(at.x, at.y - px(ABOVE_POINTER))
}

fn without_stray_separators(entries: Vec<Entry>) -> Vec<Entry> {
    let mut clean: Vec<Entry> = Vec::new();
    for entry in entries {
        let separator = matches!(entry, Entry::Separator);
        if separator && clean.last().is_none_or(|e| matches!(e, Entry::Separator)) {
            continue;
        }
        clean.push(entry);
    }
    while clean.last().is_some_and(|e| matches!(e, Entry::Separator)) {
        clean.pop();
    }
    clean
}

impl Root {
    pub(crate) fn open_menu(
        &mut self,
        at: Point<Pixels>,
        entries: Vec<Entry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_menu((at, false), entries, window, cx);
    }

    fn show_menu(
        &mut self,
        (at, from_bottom): (Point<Pixels>, bool),
        entries: Vec<Entry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = without_stray_separators(entries);
        if entries.is_empty() {
            return;
        }
        let focus = cx.focus_handle();
        window.focus(&focus);
        window.prevent_default();
        self.side.menu = Some(Menu { at, entries, focus, highlight: None, from_bottom, opener: None });
        cx.notify();
    }

    pub(crate) fn open_menu_above(
        &mut self,
        pointer: Point<Pixels>,
        entries: Vec<Entry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_menu((above(pointer), true), entries, window, cx);
    }

    pub(crate) fn open_menu_above_from(
        &mut self,
        (opener, pointer): (&'static str, Point<Pixels>),
        entries: Vec<Entry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_menu((above(pointer), true), entries, window, cx);
        if let Some(menu) = &mut self.side.menu {
            menu.opener = Some(opener);
        }
    }

    pub(crate) fn menu_from(&self, opener: &str) -> bool {
        self.side.menu.as_ref().is_some_and(|menu| menu.opener == Some(opener))
    }

    pub(crate) fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.side.menu.take().is_some() {
            window.focus(&self.focus);
            cx.notify();
        }
    }

    fn open_submenu(&mut self, entries: &[Entry], cx: &mut Context<Self>) {
        if let Some(menu) = &mut self.side.menu {
            menu.entries = entries.to_vec();
            menu.highlight = None;
            cx.notify();
        }
    }

    fn menu_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        match key {
            "escape" => self.close_menu(window, cx),
            "down" | "up" => {
                if let Some(menu) = &mut self.side.menu {
                    menu.highlight = step(&menu.entries, menu.highlight, key == "down");
                    cx.notify();
                }
            }
            "enter" | "space" => self.choose_highlighted(window, cx),
            _ => {}
        }
    }

    fn choose_highlighted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.side.menu.as_ref().and_then(|m| m.entries.get(m.highlight?).cloned()) else { return };
        match entry {
            Entry::Item { action, .. } => {
                self.close_menu(window, cx);
                action(self, window, cx);
            }
            Entry::Submenu { entries, .. } => {
                self.open_submenu(&entries, cx);
                if let Some(menu) = &mut self.side.menu {
                    menu.highlight = step(&menu.entries, None, true);
                }
            }
            Entry::Heading(_) | Entry::Resting { .. } | Entry::Note(_) | Entry::Separator => {}
        }
    }

    pub(crate) fn render_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let menu = self.side.menu.as_ref()?;
        let list = menu_frame(&menu.focus, cx)
            .children(
                menu.entries
                    .iter()
                    .enumerate()
                    .map(|(i, entry)| menu_entry(i, entry.clone(), menu.highlight == Some(i), cx)),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        let id = SharedString::from(format!("menu-{:?}", menu.at));
        let list = motion::slide(list, id, Way::Pop, (0., -MENU_DROP));
        let corner = if menu.from_bottom { Corner::BottomLeft } else { Corner::TopLeft };
        let place = anchored().position(menu.at).anchor(corner).snap_to_window_with_margin(px(8.));
        Some(deferred(place.child(list)).with_priority(2))
    }
}

fn menu_frame(focus: &FocusHandle, cx: &mut Context<Root>) -> Stateful<Div> {
    glass::card(radius::card())
        .id("context-menu")
        .track_focus(focus)
        .min_w(px(200.))
        .max_w(px(320.))
        .p(px(4.))
        .flex()
        .flex_col()
        .text_size(px(12.))
        .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
            let held = keymap::shift_held(&e.keystroke, window);
            if keymap::chord(&e.keystroke, held, Os::CURRENT, false).is_none() {
                this.menu_key(e.keystroke.key.as_str(), window, cx);
                cx.stop_propagation();
            }
        }))
        .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_menu(window, cx)))
}

fn menu_entry(index: usize, entry: Entry, highlighted: bool, cx: &mut Context<Root>) -> AnyElement {
    let id = SharedString::from(format!("menu-{index}"));
    match entry {
        Entry::Separator => div().mx(px(10.)).my(px(4.)).h(px(1.)).bg(colours::border()).into_any_element(),
        Entry::Heading(label) => menu_heading(label).into_any_element(),
        Entry::Resting { label } => resting_row(label).into_any_element(),
        Entry::Note(text) => ui::dim(text.clone()).px(px(10.)).py(px(5.)).into_any_element(),
        Entry::Item { label, action, danger, chosen, icon: glyph, keys } => {
            item_row(id, label, (glyph, keys), (highlighted, danger))
                .when(chosen, |d| d.child(icon(Icon::Check, 12.).text_color(colours::text())))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.close_menu(window, cx);
                    action(this, window, cx);
                }))
                .into_any_element()
        }
        Entry::Submenu { label, entries } => menu_row(id, label, true, (highlighted, false))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.open_submenu(&entries, cx)))
            .into_any_element(),
    }
}

fn menu_heading(label: SharedString) -> Div {
    div()
        .px(px(10.))
        .pt(px(3.))
        .pb(px(2.))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colours::muted())
        .flex()
        .child(ui::one_line(label))
}

fn resting_row(label: SharedString) -> Div {
    div().px(px(10.)).py(px(5.)).text_color(colours::faint()).child(label)
}

fn row_base(id: SharedString, (highlighted, danger): (bool, bool)) -> (Stateful<Div>, Rgba, Rgba) {
    let (lit, ink, lit_ink) = if danger {
        (glass::tint(colours::failed(), 0.16).into(), colours::failed(), colours::failed())
    } else {
        (glass::cobalt(), colours::text(), colours::on_brand())
    };
    let base = div()
        .id(id)
        .px(px(10.))
        .py(px(5.))
        .rounded(px(radius::control()))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .cursor_pointer()
        .text_color(ink)
        .when(highlighted, |d| d.bg(lit).text_color(lit_ink))
        .hover(|s| s.bg(lit).text_color(lit_ink));
    (base, ink, lit_ink)
}

pub(crate) fn menu_row(id: SharedString, label: SharedString, more: bool, state: (bool, bool)) -> Stateful<Div> {
    row_base(id, state)
        .0
        .child(div().min_w_0().flex().child(ui::one_line(label)))
        .when(more, |d| d.child(div().text_color(colours::muted()).child("›")))
}

fn item_row(
    id: SharedString,
    label: SharedString,
    (glyph, keys): (Option<Icon>, Option<SharedString>),
    state: (bool, bool),
) -> Stateful<Div> {
    if glyph.is_none() && keys.is_none() {
        return menu_row(id, label, false, state);
    }
    let (base, ink, lit_ink) = row_base(id, state);
    let ink = if state.0 { lit_ink } else { ink };
    let named = div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(8.))
        .children(glyph.map(|glyph| icon(glyph, 14.).flex_none().text_color(ink)))
        .child(ui::one_line(label));
    base.child(named).children(
        keys.map(|keys| div().flex_none().text_color(if state.0 { lit_ink } else { colours::muted() }).child(keys)),
    )
}

#[cfg(test)]
pub(crate) fn read_entries(entries: &[Entry]) -> Vec<(&'static str, String)> {
    entries
        .iter()
        .map(|entry| match entry {
            Entry::Item { label, danger: true, .. } => ("danger", label.to_string()),
            Entry::Item { label, .. } => ("item", label.to_string()),
            Entry::Submenu { label, .. } => ("submenu", label.to_string()),
            Entry::Heading(label) => ("heading", label.to_string()),
            Entry::Resting { label } => ("resting", label.to_string()),
            Entry::Note(text) => ("note", text.to_string()),
            Entry::Separator => ("-", String::new()),
        })
        .collect()
}

#[cfg(test)]
mod tests;
