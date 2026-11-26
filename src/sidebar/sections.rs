use gpui::{
    AnyElement, App, ClickEvent, Context, Div, InteractiveElement, IntoElement, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement, Styled, div, px,
};

use super::rows::fold_glyph;
use super::{Arrangement, Fold, section_head, title_label};
use crate::app::Root;
use crate::cli::Feature;
use crate::model::{Section, SectionFolds, SectionTags};
use crate::theme::{colours, radius};
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentsRoom {
    Hidden,
    Split,
    Rest,
    Title,
}

impl AgentsRoom {
    fn of(shown: bool, folded: bool, lists_open: bool) -> Self {
        match (shown, folded, lists_open) {
            (false, ..) => Self::Hidden,
            (true, true, _) => Self::Title,
            (true, false, true) => Self::Split,
            (true, false, false) => Self::Rest,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Room {
    pub stack_drags: bool,
    pub agents: AgentsRoom,
}

impl Room {
    pub(super) fn of(arrangement: Arrangement, folds: SectionFolds, agents_section: bool) -> Self {
        let stacked = matches!(arrangement, Arrangement::ProjectsOnTop | Arrangement::WorkspacesOnTop);
        let lists_open = match arrangement {
            Arrangement::Tree => !folds.projects,
            Arrangement::ProjectsOnTop | Arrangement::WorkspacesOnTop => !(folds.projects && folds.workspaces),
            Arrangement::SideBySide => !folds.workspaces,
        };
        Self {
            stack_drags: stacked && !folds.projects && !folds.workspaces,
            agents: AgentsRoom::of(agents_section, folds.agents, lists_open),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Figures<'a> {
    count: Option<usize>,
    mark: Option<&'a str>,
}

impl<'a> Figures<'a> {
    fn of(section: Section, folded: bool, counts: bool, tags: SectionTags<'a>) -> Self {
        let counted = section == Section::Agents || (folded && counts);
        Self { count: counted.then_some(tags.count), mark: tags.attention.filter(|_| folded) }
    }
}

fn arrow(folded: bool) -> Div {
    let glyph = div().w(px(14.)).flex_none().text_size(px(13.)).line_height(px(14.));
    glyph.text_color(colours::faint()).child(fold_glyph(!folded))
}

fn figures_end(figures: Figures<'_>) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .children(figures.count.map(|count| ui::dim(count.to_string())))
        .children(figures.mark.map(|mark| ui::still_badge(Some(mark))))
}

fn title_target(section: Section, folded: bool, cx: &mut Context<Root>) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("section-{}", section.name())))
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(6.))
        .py(px(2.))
        .rounded(px(radius::control()))
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.set_folded(Fold::Section(section), !folded);
            cx.notify();
        }))
        .child(arrow(folded))
        .child(div().flex_1().min_w_0().child(title_label(ui::one_line(section.title()))))
}

fn foldable_head(target: Stateful<Div>, button: Option<Stateful<Div>>) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .px(px(10.))
        .pt(px(6.))
        .pb(px(4.))
        .child(target)
        .children(button.map(|button| div().flex_none().pl(px(4.)).pr(px(2.)).py(px(2.)).child(button)))
}

fn fixed_head(section: Section, count: usize, button: Option<Stateful<Div>>) -> Div {
    let end: AnyElement = match (section, button) {
        (Section::Agents, _) => ui::dim(count.to_string()).into_any_element(),
        (_, Some(button)) => button.into_any_element(),
        (_, None) => div().into_any_element(),
    };
    section_head(section.title(), end)
}

impl Root {
    pub(super) fn section_folds(&self, cx: &App) -> Option<SectionFolds> {
        let lacked = self.store.read(cx).releases().lacks(Feature::SectionFolds);
        lacked.is_none().then(|| self.session.as_ref().map(|session| session.collapsed_sections).unwrap_or_default())
    }

    pub(super) fn titled(
        &self,
        section: Section,
        tags: SectionTags<'_>,
        button: Option<Stateful<Div>>,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(folds) = self.section_folds(cx) else { return fixed_head(section, tags.count, button) };
        let folded = folds.folded(section);
        let figures = Figures::of(section, folded, self.side.look.current.counts, tags);
        foldable_head(title_target(section, folded, cx).child(figures_end(figures)), button)
    }
}

#[cfg(test)]
mod tests;
