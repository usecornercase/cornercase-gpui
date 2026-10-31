use gpui::{AnyElement, Context, Div, IntoElement, SharedString, Window, div, prelude::*, px};

use super::data::{self, Described, Found};
use super::pages::page_icon;
use super::{SettingsPanel, widgets};
use crate::theme::colours;
use crate::ui::{self, Icon};

fn caption_of(found: &Found) -> String {
    match found.section.title.as_str() {
        "" => found.page.clone(),
        title => format!("{} › {title}", found.page),
    }
}

fn count_text(total: usize, pages: usize) -> String {
    match (total, pages) {
        (0, 0) => "No setting matches. Try another word: what it does, or its key, such as fetch_minutes.".into(),
        (0, _) => "No setting matches, but these pages may hold what you look for.".into(),
        (1, _) => "1 setting matches. Change it here, or open its page.".into(),
        (n, _) => format!("{n} settings match. Change them here, or open their page."),
    }
}

fn page_link(page: String, cx: &mut Context<SettingsPanel>) -> AnyElement {
    let about = data::about(&page);
    ui::row(SharedString::from(format!("settings-goto-{page}")), false)
        .gap(px(12.))
        .py(px(9.))
        .child(ui::icon(page_icon(&page), 15.).text_color(colours::accent()))
        .child(widgets::label(page.clone(), about).flex_1().min_w_0())
        .child(ui::icon(Icon::ChevronRight, 13.).text_color(colours::faint()))
        .on_click(cx.listener(move |this, _, _, cx| this.go_to(page.clone(), cx)))
        .into_any_element()
}

impl SettingsPanel {
    pub(super) fn render_search(
        &self,
        described: &Described,
        query: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<Div> {
        let found = data::search(described, query);
        let mut pages = data::pages_matching(&self.all_pages(), query);
        pages.retain(|page| data::count(&found, page) == 0);
        let total = found.iter().map(|f| f.section.items.len()).sum();
        let title = format!("Results for “{query}”");
        let mut parts = vec![widgets::heading(Icon::Search, title, count_text(total, pages.len()))];
        if !pages.is_empty() {
            let links: Vec<AnyElement> = pages.into_iter().map(|page| page_link(page, cx)).collect();
            parts.push(widgets::group("Pages", div().flex().flex_col().gap(px(2.)).children(links)));
        }
        for f in &found {
            parts.push(widgets::group(caption_of(f), self.section_body(described, &f.section.items, window, cx)));
        }
        parts
    }
}
