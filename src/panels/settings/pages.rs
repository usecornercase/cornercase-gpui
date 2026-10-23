use gpui::{AnyElement, Context, Div, IntoElement, SharedString, Window, div, prelude::*, px};

use super::data::{self, Described, Item, Section, Setting};
use super::{Role, SettingsPanel, widgets};
use crate::theme::colours;
use crate::ui;

#[derive(Clone, Copy)]
pub(super) struct Slot<'w> {
    pub first: bool,
    pub window: &'w Window,
}

pub(super) struct AgentItem<'d> {
    pub kind: &'d str,
    pub mode: Option<&'d Setting>,
    pub extra: Option<&'d Setting>,
}

fn about(page: &str) -> &'static str {
    match page {
        "Worktrees" => "Where worktrees go, how issue branches are named, and fetching.",
        "Agents" => "The agent that starts by default, and how each one starts.",
        "Issues" => "The trackers issues come from, their accounts and filters.",
        "UI" => "How the sidebar, tabs and notifications look and sound.",
        "Remote" => "Reaching this computer's cornercase from a phone or a browser.",
        _ => "",
    }
}

fn home_relative(path: String) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    match path.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => path,
    }
}

fn problem_banner(problem: &str) -> Div {
    div()
        .px(px(12.))
        .py(px(8.))
        .rounded(px(6.))
        .border_1()
        .border_color(colours::failed())
        .text_color(colours::failed())
        .child(format!("config.json cannot be read, so nothing is saved until it is fixed: {problem}"))
}

impl SettingsPanel {
    pub(super) fn render_nav(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut pages = self.described.as_ref().map(data::pages).unwrap_or_default();
        let settings_pages = pages.len();
        pages.push(data::PROJECTS.into());
        pages.push(data::SHARE.into());
        let path = home_relative(self.described.as_ref().map(|d| d.path.clone()).unwrap_or_default());
        let items: Vec<Div> = pages
            .into_iter()
            .enumerate()
            .map(|(i, page)| {
                div()
                    .when(i == settings_pages, |d| d.mt(px(8.)).pt(px(8.)).border_t_1().border_color(colours::border()))
                    .child(self.nav_item(page, cx))
            })
            .collect();
        widgets::nav().children(items).child(div().flex_1()).child(ui::dim(path).px(px(8.)))
    }

    fn nav_item(&self, page: String, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let selected = page == self.page;
        widgets::nav_item(SharedString::from(format!("settings-page-{page}")), page.clone(), selected).on_click(
            cx.listener(move |this, _, _, cx| {
                this.page.clone_from(&page);
                this.open_choice = None;
                this.pending = None;
                cx.notify();
            }),
        )
    }

    pub(super) fn render_page(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(described) = &self.described else {
            return match &self.load_error {
                Some(e) => ui::note(format!("Could not read the settings: {e}")).into_any_element(),
                None => ui::note("Reading the settings…").into_any_element(),
            };
        };
        let page = div().flex().flex_col().gap(px(14.)).children(described.problem.as_deref().map(problem_banner));
        match self.page.as_str() {
            data::PROJECTS => page.child(self.render_projects(described, window, cx)).into_any_element(),
            data::SHARE => page.child(self.render_share(cx)).into_any_element(),
            name => page.children(self.settings_page(described, name, window, cx)).into_any_element(),
        }
    }

    fn settings_page(&self, described: &Described, name: &str, window: &Window, cx: &mut Context<Self>) -> Vec<Div> {
        let mut parts = vec![widgets::heading(name.to_string(), about(name))];
        for section in data::sections(described, name) {
            parts.push(self.section(described, &section, window, cx));
        }
        parts
    }

    fn section(&self, described: &Described, section: &Section, window: &Window, cx: &mut Context<Self>) -> Div {
        let mut card = widgets::card();
        for (i, item) in section.items.iter().enumerate() {
            card = card.child(self.render_item(described, item, Slot { first: i == 0, window }, cx));
        }
        div()
            .flex()
            .flex_col()
            .when(!section.title.is_empty(), |d| d.child(ui::section(section.title.clone())))
            .child(card)
    }

    fn render_item(&self, d: &Described, item: &Item, slot: Slot<'_>, cx: &mut Context<Self>) -> AnyElement {
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
        let label = widgets::label(widgets::capitalised(&setting.label), setting.note.clone());
        let control = self.render_control(setting, slot.window, cx);
        let reset = (!setting.is_default() && !setting.default.is_null() && !self.busy.contains(&role)).then(|| {
            let (key, value) = (setting.key.clone(), data::value_text(&setting.default));
            ui::icon_button(SharedString::from(format!("reset-{key}")), "↺")
                .on_click(cx.listener(move |this, _, _, cx| this.save(&key, &value, cx)))
        });
        div()
            .flex()
            .flex_col()
            .child(widgets::row(
                slot.first,
                label,
                div().flex().items_center().gap(px(6.)).child(control).children(reset),
            ))
            .children(self.render_choice_list(setting, cx))
            .children(self.render_pending_choice(setting, cx))
            .children(self.error_of(&role))
            .into_any_element()
    }
}
