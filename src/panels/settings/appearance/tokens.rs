use gpui::{
    AnyElement, Context, Div, IntoElement as _, ParentElement as _, Rgba, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::picker::Spot;
use super::{Snap, widgets};
use crate::appearance::{Appearance, Part};
use crate::notify::tip;
use crate::panels::settings::SettingsPanel;
use crate::theme::colour::{from_bytes, hex};
use crate::theme::contrast::{self, Check};
use crate::theme::resolve::Theme;
use crate::theme::tokens::{ANSI_NAMES, Group, TOKENS, Token};
use crate::theme::{FONT_FAMILY, colours};
use crate::ui::{self, Icon};

struct Row<'a> {
    label: &'a str,
    note: &'a str,
    key: &'a str,
    part: Part,
    colour: Rgba,
    base: Option<Rgba>,
}

pub(super) fn terminal_colour(theme: &Theme, key: &str) -> Option<Rgba> {
    let terminal = &theme.terminal;
    let bytes = match key {
        "foreground" => terminal.foreground,
        "background" => terminal.background,
        "cursor" => terminal.cursor,
        _ => terminal.ansi[ANSI_NAMES.iter().position(|name| *name == key)?],
    };
    Some(from_bytes(bytes))
}

fn chrome_token_row<'a>(snap: &Snap, token: &'a Token) -> Row<'a> {
    let colour = snap.theme.chrome.get(token.key).unwrap_or(colours::text());
    let base = snap.base.as_ref().and_then(|base| base.chrome.get(token.key));
    Row { label: token.label, note: token.note, key: token.key, part: Part::Chrome, colour, base }
}

impl SettingsPanel {
    pub(super) fn colour_groups(&self, snap: &Snap, _window: &Window, cx: &mut Context<Self>) -> Vec<Div> {
        let mut groups = vec![self.terminal_group(snap, cx)];
        groups.extend(Group::ALL.into_iter().map(|group| self.chrome_group(snap, group, cx)));
        groups
    }

    fn chrome_group(&self, snap: &Snap, group: Group, cx: &mut Context<Self>) -> Div {
        let rows: Vec<AnyElement> = TOKENS
            .iter()
            .filter(|token| token.group == group)
            .enumerate()
            .map(|(i, token)| self.colour_row(i == 0, &chrome_token_row(snap, token), snap, cx))
            .collect();
        widgets::group(group.caption(), widgets::card().children(rows))
    }

    fn terminal_group(&self, snap: &Snap, cx: &mut Context<Self>) -> Div {
        let named = [("foreground", "Text"), ("background", "Background"), ("cursor", "Cursor")];
        let rows: Vec<AnyElement> = named
            .into_iter()
            .enumerate()
            .filter_map(|(i, (key, label))| {
                let colour = terminal_colour(&snap.theme, key)?;
                let base = snap.base.as_ref().and_then(|base| terminal_colour(base, key));
                let row = Row { label, note: "", key, part: Part::Terminal, colour, base };
                Some(self.colour_row(i == 0, &row, snap, cx))
            })
            .collect();
        let note = snap.copy_of.as_ref().map(|name| {
            let line = format!("Editing a copy of {name}: the first change makes your own theme in the themes folder.");
            div().pb(px(8.)).child(widgets::hint(line))
        });
        let selection = widgets::hint(
            "The terminal's selection colour waits on cornercase: it cannot yet tell selected cells apart from \
             inverse text.",
        );
        let body =
            widgets::card().children(rows).child(self.ansi_rows(snap, cx)).child(div().py(px(10.)).child(selection));
        widgets::group("Terminal", div().flex().flex_col().children(note).child(body))
    }

    fn ansi_rows(&self, snap: &Snap, cx: &mut Context<Self>) -> Div {
        let line = |names: &[&str], cx: &mut Context<Self>| {
            let swatches: Vec<Div> = names
                .iter()
                .filter_map(|key| {
                    let colour = terminal_colour(&snap.theme, key)?;
                    Some(self.swatch_button(&Spot { key, part: Part::Terminal, colour, size: 26. }, snap, cx))
                })
                .collect();
            div().flex().gap(px(6.)).children(swatches)
        };
        let (normal, bright) = ANSI_NAMES.split_at(8);
        div()
            .py(px(12.))
            .border_t_1()
            .border_color(colours::border())
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(widgets::label(
                "The 16 colours",
                "What programs draw with: Claude Code, Neovim and delta follow them.",
            ))
            .child(line(normal, cx))
            .child(line(bright, cx))
    }

    fn colour_row(&self, first: bool, row: &Row<'_>, snap: &Snap, cx: &mut Context<Self>) -> AnyElement {
        let spot = Spot { key: row.key, part: row.part, colour: row.colour, size: 24. };
        let hex_text = div()
            .w(px(78.))
            .text_size(px(11.5))
            .text_color(colours::muted())
            .font_family(FONT_FAMILY)
            .child(hex(row.colour, row.colour.a < 1.));
        let control = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(self.swatch_button(&spot, snap, cx))
            .child(hex_text)
            .children(Self::contrast_chip(row, snap, cx));
        widgets::row(
            first,
            widgets::label(row.label.to_string(), row.note.to_string()),
            control,
            Self::reset_slot(row, cx),
        )
        .into_any_element()
    }

    fn reset_slot(row: &Row<'_>, cx: &mut Context<Self>) -> AnyElement {
        let Some(base) = row.base.filter(|base| *base != row.colour) else { return div().into_any_element() };
        let (key, part) = (row.key.to_string(), row.part);
        let hint = format!("Back to the base's: {}", hex(base, base.a < 1.));
        ui::row_action(SharedString::from(format!("reset-colour-{key}")), Icon::Undo, hint, false)
            .on_click(cx.listener(move |_, _, _, cx| {
                Appearance::get(cx).update(cx, |appearance, cx| appearance.set_colour(part, &key, None, cx));
            }))
            .into_any_element()
    }

    fn contrast_chip(row: &Row<'_>, snap: &Snap, cx: &mut Context<Self>) -> Option<AnyElement> {
        let check = contrast::check_of(&snap.theme, row.key)?;
        let key = row.key.to_string();
        let text = format!("{:.1} : 1", check.ratio);
        let chip = ui::tinted_chip(text, if check.passes() { colours::done() } else { colours::failed() });
        let id = SharedString::from(format!("contrast-{key}"));
        let chip = gpui::InteractiveElement::id(chip, id).tooltip(tip(chip_tip(&check)));
        let fix = (!check.passes()).then(|| {
            ui::button(SharedString::from(format!("fix-{key}")), "Fix").on_click(cx.listener(move |_, _, _, cx| {
                fix_colour(&key, cx);
            }))
        });
        Some(div().flex().items_center().gap(px(6.)).child(chip).children(fix).into_any_element())
    }
}

fn chip_tip(check: &Check) -> String {
    let verdict = if check.passes() { "is readable" } else { "is hard to read" };
    format!("{:.1} : 1 over {}, needs {} : 1: it {verdict}", check.ratio, check.surface, check.needed)
}

fn fix_colour(key: &str, cx: &mut gpui::App) {
    let theme = Appearance::get(cx).read(cx).in_use();
    let Some((fixed, _)) = contrast::fix_token(&theme, key) else { return };
    let key = key.to_string();
    Appearance::get(cx).update(cx, |appearance, cx| appearance.set_colour(Part::Chrome, &key, Some(fixed), cx));
}
