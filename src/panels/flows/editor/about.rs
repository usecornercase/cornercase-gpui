use gpui::{ClickEvent, Context, Div, ParentElement as _, Styled as _, Window, div, px};

use super::super::draft::{Place, place_note};
use super::super::editor_view::{adder, choice, labelled, line, text_control};
use super::super::flow_page::block;
use super::{FlowEditor, Layout};
use crate::ui::{self, Icon};

impl FlowEditor {
    pub(in crate::panels::flows) fn about(&self, layout: Layout, window: &Window, cx: &Context<Self>) -> Div {
        block(Icon::File, "About")
            .child(self.name_and_description(layout, window, cx))
            .children((self.new && self.project.is_some()).then(|| self.save_in(cx)))
            .child(self.notes_part(window, cx))
    }

    fn name_and_description(&self, layout: Layout, window: &Window, cx: &Context<Self>) -> Div {
        if layout.about_in_a_row() {
            let name = text_control("Name", &self.name, window, cx).w(px(210.)).flex_none();
            return line().child(name).child(labelled("Description", &self.description, window, cx));
        }
        div().flex().flex_col().gap(px(10.)).child(text_control("Name", &self.name, window, cx)).child(text_control(
            "Description",
            &self.description,
            window,
            cx,
        ))
    }

    fn save_in(&self, cx: &Context<Self>) -> Div {
        let pick = |id: &'static str, label: &'static str, place: Place| {
            let selected = self.place == place;
            choice(id, label, selected, cx, move |this, _, cx| this.set_place(place.clone(), cx))
        };
        let segments = ui::segments(vec![
            pick("editor-yours", "Your flows", Place::Yours),
            pick("editor-project", "This project", Place::Project),
        ]);
        let row = div().flex().items_center().gap(px(10.)).child(segments).child(ui::dim(place_note(&self.place)));
        ui::field("Save in", row)
    }

    fn notes_part(&self, window: &Window, cx: &Context<Self>) -> Div {
        if self.notes_open {
            return text_control("Notes for the orchestrator", &self.notes, window, cx);
        }
        adder(
            "editor-notes",
            Icon::Plus,
            "Notes for the orchestrator",
            true,
            cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_notes(window, cx)),
        )
    }
}
