use gpui::{Div, Rgba};

use crate::model::{Block, Layer, PullRequest};
use crate::theme::colours;
use crate::ui::{self, Icon};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sign {
    Failed,
    Pending,
    Passed,
    Conflicts,
    Quiet,
}

impl Sign {
    pub(crate) fn of_checks(checks: Option<&str>) -> Self {
        match checks {
            Some("failing") => Self::Failed,
            Some("pending") => Self::Pending,
            Some("passing") => Self::Passed,
            _ => Self::Quiet,
        }
    }

    pub(crate) fn glyph(self) -> &'static str {
        match self {
            Self::Failed => "✗",
            Self::Pending => "•",
            Self::Passed => "✓",
            Self::Conflicts => "!",
            Self::Quiet => "",
        }
    }

    pub(crate) fn colour(self) -> Rgba {
        match self {
            Self::Failed => colours::failed(),
            Self::Pending => colours::working(),
            Self::Passed => colours::done(),
            Self::Conflicts => colours::waiting(),
            Self::Quiet => colours::muted(),
        }
    }

    pub(super) fn icon(self) -> Option<Icon> {
        match self {
            Self::Failed => Some(Icon::Failed),
            Self::Pending => Some(Icon::Pending),
            Self::Passed => Some(Icon::Done),
            Self::Conflicts => Some(Icon::Waiting),
            Self::Quiet => None,
        }
    }
}

fn open_sign(conflicts: bool, checks: Option<&str>) -> Sign {
    if conflicts { Sign::Conflicts } else { Sign::of_checks(checks) }
}

pub(crate) fn pull_tag(pr: &PullRequest) -> (String, Rgba, bool) {
    let number = format!("#{}", pr.number);
    match pr.state.as_str() {
        "merged" => (number, colours::merged(), false),
        "closed" => (number, colours::faint(), true),
        _ => {
            let sign = open_sign(pr.conflicts, pr.checks.as_deref());
            (format!("{number}{}", sign.glyph()), sign.colour(), false)
        }
    }
}

pub(crate) fn block_colour(block: Block) -> Rgba {
    match block {
        Block::Checks => colours::failed(),
        Block::Conflicts => colours::waiting(),
        Block::Review => colours::muted(),
    }
}

pub(crate) fn stack_tag(pr: &PullRequest) -> Option<(String, Rgba)> {
    let stack = pr.stack.as_ref()?;
    let colour = pr.blocked().map_or(colours::muted(), |(_, block)| block_colour(block));
    Some((format!("{}/{}", stack.position, stack.size), colour))
}

pub(super) fn layer_sign(pr: &PullRequest, layer: &Layer, own: bool) -> Sign {
    let (conflicts, checks) =
        if own { (pr.conflicts, pr.checks.as_deref()) } else { (layer.conflicts, layer.checks.as_deref()) };
    if layer.settled() { Sign::Quiet } else { open_sign(conflicts, checks) }
}

pub(super) fn state_chip(pr: &PullRequest) -> Div {
    match pr.state.as_str() {
        "open" => ui::tinted_chip("open", colours::done()),
        "merged" => ui::tinted_chip("merged", colours::merged()),
        state => ui::quiet_chip(state.to_string()),
    }
}

pub(super) fn checks_chip(pr: &PullRequest) -> Div {
    let sign = Sign::of_checks(pr.checks.as_deref());
    if pr.settled() || sign == Sign::Quiet {
        return ui::quiet_chip(pr.checks_said());
    }
    ui::tinted_chip(pr.checks_said(), sign.colour())
}

fn rollup_line(sign: Sign, gitlab: bool) -> &'static str {
    let [on_github, on_gitlab] = match sign {
        Sign::Failed => ["Some checks failed", "The pipeline failed"],
        Sign::Pending => ["Checks are running", "The pipeline is running"],
        Sign::Passed => ["Every check passed", "The pipeline passed"],
        Sign::Conflicts | Sign::Quiet => ["No checks", "No pipeline"],
    };
    if gitlab { on_gitlab } else { on_github }
}

pub(super) fn check_lines(pr: &PullRequest) -> Vec<(Sign, String)> {
    if !pr.failed.is_empty() {
        return pr.failed.iter().map(|name| (Sign::Failed, name.clone())).collect();
    }
    let sign = Sign::of_checks(pr.checks.as_deref());
    vec![(sign, rollup_line(sign, pr.on_gitlab()).to_string())]
}

#[cfg(test)]
mod tests;
