use gpui::{
    AnyElement, ClickEvent, Context, Corner, Div, Focusable as _, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    anchored, deferred, div, point, px,
};

use super::IssuesPanel;
use super::data::{Who, capitalise};
use crate::theme::{colours, radius};
use crate::ui::motion::{self, Way};
use crate::ui::{self, Icon, glass};

const PEOPLE_SHOWN: usize = 80;
const DROP: f32 = 4.;

struct Pick {
    index: usize,
    who: Who,
}

impl IssuesPanel {
    pub(super) fn picker_popover(&self, index: usize, field: &str, window: &Window, cx: &mut Context<Self>) -> Div {
        let card =
            motion::slide(self.picker_card(index, field, window, cx), ("picker-in", index), Way::Pop, (0., -DROP));
        div().absolute().top_full().right_0().child(
            deferred(
                anchored()
                    .anchor(Corner::TopRight)
                    .offset(point(px(0.), px(6.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(card),
            )
            .with_priority(1),
        )
    }

    fn picker_card(&self, index: usize, field: &str, window: &Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let (rows, note) = self.who_rows(index, cx);
        let card = glass::card(radius::CARD)
            .id("issues-picker")
            .occlude()
            .w(px(280.))
            .p(px(4.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .text_size(px(12.))
            .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| cx.stop_propagation())
            .child(ui::section(capitalise(field)).px(px(10.)).pt(px(6.)).pb(px(4.)));
        let focused = self.picker_search.read(cx).focus_handle(cx).is_focused(window);
        let search = self.tab.source().map(|_| {
            div().px(px(2.)).pb(px(4.)).child(ui::search_field(self.picker_search.clone(), focused, None).py(px(4.)))
        });
        card.children(search)
            .child(div().id("issues-picker-rows").max_h(px(300.)).overflow_y_scroll().flex().flex_col().children(rows))
            .children(note)
    }

    fn who_rows(&self, index: usize, cx: &mut Context<Self>) -> (Vec<AnyElement>, Option<Div>) {
        let current = self.who(&self.tab)[index].clone();
        let mut rows: Vec<AnyElement> = [(Who::Anyone, "Anyone"), (Who::Me, "Me")]
            .into_iter()
            .map(|(who, label)| {
                let selected = current == who;
                let id = SharedString::from(format!("who-{label}"));
                who_row(id, (label.to_string(), None), selected, Pick { index, who }, cx)
            })
            .collect();
        let note = self.people_rows(index, &current, &mut rows, cx);
        (rows, note)
    }

    fn people_rows(
        &self,
        index: usize,
        current: &Who,
        rows: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let Some(source) = self.tab.source() else {
            return Some(note("Pick a tracker's tab to filter by someone in particular."));
        };
        let people = match self.people_lists.get(source) {
            None => {
                return Some(ui::busy("issues-people", "Asking the tracker for its people…").px(px(10.)).py(px(6.)));
            }
            Some(Err(e)) => return Some(note(e.clone()).text_color(colours::failed())),
            Some(Ok(people)) => people,
        };
        let matching = people.matching(&self.picker_search.read(cx).text());
        if matching.is_empty() {
            return Some(note("Nobody matches."));
        }
        rows.push(div().mx(px(10.)).my(px(4.)).h(px(1.)).bg(colours::border()).into_any_element());
        for (i, person) in matching.iter().take(PEOPLE_SHOWN).enumerate() {
            let who = person.who();
            let selected = *current == who;
            rows.push(who_row(
                SharedString::from(format!("who-p-{i}")),
                person.display(),
                selected,
                Pick { index, who },
                cx,
            ));
        }
        None
    }
}

fn note(text: impl IntoElement) -> Div {
    ui::dim(text).px(px(10.)).py(px(6.))
}

fn who_row(
    id: SharedString,
    (name, handle): (String, Option<String>),
    selected: bool,
    pick: Pick,
    cx: &mut Context<IssuesPanel>,
) -> AnyElement {
    let Pick { index, who } = pick;
    let group = SharedString::from(format!("{id}-row"));
    let check = selected.then(|| {
        ui::icon(Icon::Check, 13.)
            .text_color(colours::accent())
            .group_hover(group.clone(), |s| s.text_color(colours::on_brand()))
    });
    div()
        .id(id)
        .group(group.clone())
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(10.))
        .py(px(5.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .text_color(colours::text())
        .hover(|s| s.bg(glass::cobalt()).text_color(colours::on_brand()))
        .child(div().w(px(13.)).flex_none().children(check))
        .child(div().flex_1().min_w_0().child(ui::one_line(name)))
        .children(handle.map(|h| {
            div()
                .flex_none()
                .text_color(colours::faint())
                .group_hover(group, |s| s.text_color(colours::on_brand()))
                .child(h)
        }))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_who(index, who.clone(), cx)))
        .into_any_element()
}
