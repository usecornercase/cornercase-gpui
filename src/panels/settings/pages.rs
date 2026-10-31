use gpui::{AnyElement, Context, Div, Focusable as _, IntoElement, SharedString, Window, div, prelude::*, px};

use super::agents::AgentItem;
use super::data::{self, Described, Item, Kind, Setting};
use super::{Role, SettingsPanel, widgets};
use crate::notify::tip;
use crate::theme::colours;
use crate::ui::{self, Icon, TextField};

#[derive(Clone, Copy)]
pub(super) struct Slot<'w> {
    pub first: bool,
    pub window: &'w Window,
}

#[derive(Clone, Copy)]
enum Mark {
    Plain,
    Count(usize),
    Alert,
    Dim,
}

pub(super) fn page_icon(page: &str) -> Icon {
    match page {
        "Worktrees" => Icon::Worktrees,
        "Agents" => Icon::Agent,
        "Issues" => Icon::Issues,
        "UI" => Icon::Interface,
        "Remote" => Icon::Devices,
        data::PROJECTS => Icon::FolderOpen,
        data::SHARE => Icon::Share,
        _ => Icon::Settings,
    }
}

fn setting_label(setting: &Setting) -> Div {
    let restart = setting.restart.then(|| ui::tinted_chip("needs a restart", colours::waiting()));
    widgets::labelled(widgets::titled(widgets::capitalised(&setting.label), restart), setting.note.clone())
}

impl SettingsPanel {
    pub(super) fn all_pages(&self) -> Vec<String> {
        let mut pages = self.described.as_ref().map(data::pages).unwrap_or_default();
        pages.push(data::PROJECTS.into());
        pages.push(data::SHARE.into());
        pages
    }

    pub(super) fn render_nav(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let query = self.query(cx);
        let found = self.described.as_ref().map(|d| data::search(d, &query)).unwrap_or_default();
        let pages = self.all_pages();
        let matching = data::pages_matching(&pages, &query);
        let own = pages.len().saturating_sub(2);
        let items: Vec<Div> = pages
            .into_iter()
            .enumerate()
            .map(|(i, page)| {
                let mark = self.nav_mark(&page, &query, &found, &matching);
                let selected = query.is_empty() && page == self.page;
                div()
                    .flex()
                    .flex_col()
                    .when(i == own, |d| d.mt(px(8.)).pt(px(8.)).border_t_1().border_color(colours::border()))
                    .child(Self::nav_item(page, selected, mark, cx))
            })
            .collect();
        widgets::nav(self.nav_width.size())
            .child(self.search_box(window, cx))
            .children(items)
            .child(div().flex_1())
            .children(self.config_path())
    }

    fn nav_mark(&self, page: &str, query: &str, found: &[data::Found], matching: &[String]) -> Mark {
        if query.is_empty() {
            let alert = self.described.as_ref().is_some_and(|d| data::alerts(d, page));
            return if alert { Mark::Alert } else { Mark::Plain };
        }
        match data::count(found, page) {
            0 if matching.iter().any(|p| p == page) => Mark::Plain,
            0 => Mark::Dim,
            n => Mark::Count(n),
        }
    }

    fn nav_item(page: String, selected: bool, mark: Mark, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = SharedString::from(format!("settings-page-{page}"));
        let count = match mark {
            Mark::Count(n) => Some(widgets::nav_count(n)),
            _ => None,
        };
        widgets::nav_item(id, page_icon(&page), page.clone(), selected)
            .when(matches!(mark, Mark::Dim), |d| d.opacity(0.45))
            .children(count)
            .children(matches!(mark, Mark::Alert).then(|| ui::dot(Some(colours::failed()))))
            .on_click(cx.listener(move |this, _, _, cx| this.go_to(page.clone(), cx)))
    }

    fn search_box(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.search.read(cx).focus_handle(cx).is_focused(window);
        let clear = (!self.search.read(cx).text().is_empty()).then(|| {
            ui::field_action("settings-search-clear", Icon::Close, "Clear the search", colours::muted())
                .on_click(cx.listener(|this, _, _, cx| this.search.update(cx, TextField::clear)))
        });
        div().flex().flex_col().pb(px(10.)).child(ui::search_field(self.search.clone(), focused, clear))
    }

    fn config_path(&self) -> Option<impl IntoElement + use<>> {
        let path = self.described.as_ref()?.path.clone();
        let shown = ui::home_relative(&path);
        Some(
            div()
                .id("settings-config-path")
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .pt(px(8.))
                .text_size(px(11.))
                .text_color(colours::faint())
                .tooltip(tip(format!("Settings are saved in {path}")))
                .child(ui::icon(Icon::File, 12.).text_color(colours::faint()))
                .child(div().flex_1().min_w_0().flex().child(ui::one_line(shown))),
        )
    }

    pub(super) fn render_page(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(described) = &self.described else { return self.not_loaded(cx) };
        let query = self.query(cx);
        let problem = described.problem.as_deref().map(|p| {
            ui::callout(
                colours::failed(),
                "config.json cannot be read",
                format!("Nothing is saved until it is fixed: {p}"),
            )
        });
        let column = widgets::column().children(problem);
        if !query.is_empty() {
            return column.children(self.render_search(described, &query, window, cx)).into_any_element();
        }
        let column = column.child(widgets::heading(page_icon(&self.page), self.page.clone(), data::about(&self.page)));
        match self.page.as_str() {
            data::PROJECTS => column.children(self.render_projects(described, window, cx)),
            data::SHARE => column.children(self.render_share(cx)),
            name => column.children(self.settings_sections(described, name, window, cx)),
        }
        .into_any_element()
    }

    fn not_loaded(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(error) = &self.load_error else {
            return widgets::column().child(ui::loading("Reading the settings…")).into_any_element();
        };
        let retry = if self.loading {
            ui::busy_button("settings-retry", "Reading…", ui::Tone::Plain)
        } else {
            ui::button("settings-retry", "Try again").on_click(cx.listener(|this, _, _, cx| this.load(cx)))
        };
        widgets::column()
            .child(ui::callout(colours::failed(), "Could not read the settings", error.clone()))
            .child(div().flex().child(retry))
            .into_any_element()
    }

    fn settings_sections(&self, d: &Described, page: &str, window: &Window, cx: &mut Context<Self>) -> Vec<Div> {
        let sections = data::sections(d, page);
        sections
            .iter()
            .map(|s| widgets::group(data::caption(d, s), self.section_body(d, &s.items, window, cx)))
            .collect()
    }

    pub(super) fn section_body(&self, d: &Described, items: &[Item], window: &Window, cx: &mut Context<Self>) -> Div {
        if items.iter().any(|i| matches!(i, Item::Agent { .. })) {
            return self.agents_card(d, items, window, cx);
        }
        widgets::card().children(self.item_rows(d, items, window, cx))
    }

    pub(super) fn item_rows(
        &self,
        d: &Described,
        items: &[Item],
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let slot = |i: usize| Slot { first: i == 0, window };
        items.iter().enumerate().map(|(i, item)| self.render_item(d, item, slot(i), cx)).collect()
    }

    pub(super) fn render_item(&self, d: &Described, item: &Item, slot: Slot<'_>, cx: &mut Context<Self>) -> AnyElement {
        match item {
            Item::Setting(i) => self.render_setting(&d.settings[*i], slot, cx),
            Item::Connection(i) => self.render_connection(&d.connections[*i], slot, cx),
            Item::Agent { kind, mode, extra } => {
                let agent =
                    AgentItem { kind, mode: mode.map(|i| &d.settings[i]), extra: extra.map(|i| &d.settings[i]) };
                self.render_agent(&agent, slot, cx)
            }
        }
    }

    fn render_setting(&self, setting: &Setting, slot: Slot<'_>, cx: &mut Context<Self>) -> AnyElement {
        let role = Role::Setting(setting.key.clone());
        let control = self.render_control(setting, slot.window, cx);
        let row = widgets::row(slot.first, setting_label(setting), control, self.setting_slot(setting, cx));
        div()
            .flex()
            .flex_col()
            .child(row)
            .children((setting.kind == Kind::List).then(|| self.render_sources(setting, cx)))
            .children(self.hint_of(&role, setting.hint(), slot.window, cx))
            .children(self.error_of(&role))
            .into_any_element()
    }

    fn setting_slot(&self, setting: &Setting, cx: &mut Context<Self>) -> AnyElement {
        let role = Role::Setting(setting.key.clone());
        let busy = self.busy.contains(&role);
        if busy && !setting.edited_as_text() {
            return ui::spinner(SharedString::from(format!("saving-{}", setting.key)), 13., colours::muted())
                .into_any_element();
        }
        if busy || setting.is_default() || setting.default.is_null() {
            return div().into_any_element();
        }
        let (key, value) = (setting.key.clone(), data::value_text(&setting.default));
        let hint = format!("Back to the default: {}", data::default_shown(setting));
        ui::row_action(SharedString::from(format!("reset-{key}")), Icon::Undo, hint, false)
            .on_click(cx.listener(move |this, _, _, cx| this.save(&key, &value, cx)))
            .into_any_element()
    }

    pub(super) fn hint_of(&self, role: &Role, hint: Option<&str>, window: &Window, cx: &gpui::App) -> Option<Div> {
        let hint = hint?;
        let focused = self.fields.get(role)?.read(cx).focus_handle(cx).is_focused(window);
        focused.then(|| widgets::under(widgets::hint(hint.to_string())))
    }

    pub(super) fn error_of(&self, role: &Role) -> Option<Div> {
        self.errors.get(role).map(|e| widgets::under(ui::error_line(e.clone())))
    }
}
