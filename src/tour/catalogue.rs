use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::Guide;
use super::model::Tour;
use crate::dialogs;
use crate::store::Store;
use crate::theme::colours;
use crate::ui;

const NOTE: &str = "Short guided tours of cornercase, from its own steps. They name cornercase's terminal window, \
                    whose buttons sit elsewhere here: the rail on the left opens every panel.";

struct Tours {
    guide: Entity<Guide>,
    focus: FocusHandle,
}

pub(crate) fn open(window: &mut Window, cx: &mut App) {
    let guide = Guide::get(cx);
    guide.update(cx, |_, cx| Guide::load(cx));
    let view = cx.new(|cx| {
        cx.observe(&guide, |_, _, cx| cx.notify()).detach();
        Tours { guide, focus: cx.focus_handle() }
    });
    let focus = view.read(cx).focus.clone();
    Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

impl Tours {
    fn row(&self, tour: &Tour, progress: String, cx: &mut Context<Self>) -> Div {
        let id = tour.id.clone();
        let label = if progress == "done" { "Again" } else { "Start" };
        let guide = self.guide.clone();
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .py(px(4.))
            .child(div().flex_1().child(tour.title.clone()))
            .child(ui::dim(progress))
            .child(ui::button(SharedString::from(format!("tour-{id}")), label).on_click(cx.listener(
                move |_, _: &ClickEvent, _, cx| {
                    guide.update(cx, |g, cx| g.start(&id, cx));
                    Store::get(cx).update(cx, Store::close_dialog);
                },
            )))
    }
}

impl Render for Tours {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let guide = self.guide.read(cx);
        let tours: Vec<(Tour, String)> =
            guide.catalogue.tours.iter().map(|t| (t.clone(), guide.catalogue.progress_label(t))).collect();
        let rows: Vec<Div> = tours.iter().map(|(t, p)| self.row(t, p.clone(), cx)).collect();
        let empty = rows.is_empty().then(|| ui::loading("Reading the tours…"));
        div().track_focus(&self.focus).child(dialogs::frame(
            "Tours",
            div()
                .id("tours")
                .max_h(px(420.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .child(div().pb(px(8.)).text_size(px(12.)).text_color(colours::muted()).child(NOTE))
                .children(empty)
                .children(rows),
            ui::button("tours-close", "Close").on_click(|_: &ClickEvent, _, cx| {
                Store::get(cx).update(cx, Store::close_dialog);
            }),
        ))
    }
}
