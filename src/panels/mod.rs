pub(crate) mod accounts;
pub(crate) mod browser;
pub(crate) mod changes;
pub(crate) mod files;
pub(crate) mod flows;
pub(crate) mod issues;
pub(crate) mod plugins;
pub(crate) mod poll;
pub(crate) mod ports;
pub(crate) mod processes;
pub(crate) mod queue;
pub(crate) mod search;
pub(crate) mod settings;
pub(crate) mod todo;
pub(crate) mod usage;
pub(crate) mod worktrees;

use gpui::{AnyView, Window};

use crate::model::Session;
use crate::ui::Icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PanelKind {
    Changes,
    Files,
    Todo,
    Browser,
    Flow,
    Queue,
    Issues,
    Usage,
    Settings,
    Accounts,
    Flows,
    Search,
    Plugins,
    Processes,
    Ports,
    Worktrees,
}

impl PanelKind {
    pub(crate) const ALL: [PanelKind; 16] = [
        PanelKind::Changes,
        PanelKind::Files,
        PanelKind::Todo,
        PanelKind::Browser,
        PanelKind::Flow,
        PanelKind::Queue,
        PanelKind::Issues,
        PanelKind::Usage,
        PanelKind::Settings,
        PanelKind::Accounts,
        PanelKind::Flows,
        PanelKind::Search,
        PanelKind::Plugins,
        PanelKind::Processes,
        PanelKind::Ports,
        PanelKind::Worktrees,
    ];

    pub(crate) fn icon(self) -> Icon {
        match self {
            Self::Changes => Icon::Changes,
            Self::Files => Icon::Files,
            Self::Todo => Icon::Todo,
            Self::Browser => Icon::Globe,
            Self::Flow => Icon::Waypoints,
            Self::Queue => Icon::Queue,
            Self::Issues => Icon::Issues,
            Self::Usage => Icon::Usage,
            Self::Settings => Icon::Settings,
            Self::Accounts => Icon::Accounts,
            Self::Flows => Icon::Flows,
            Self::Search => Icon::Search,
            Self::Plugins => Icon::Plugins,
            Self::Processes => Icon::Processes,
            Self::Ports => Icon::Ports,
            Self::Worktrees => Icon::Worktrees,
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Changes => "Changes",
            Self::Files => "Files",
            Self::Todo => "TODO",
            Self::Browser => "Browser",
            Self::Flow => "Flow",
            Self::Queue => "Queue",
            Self::Issues => "Issues",
            Self::Usage => "Usage",
            Self::Settings => "Settings",
            Self::Accounts => "Accounts",
            Self::Flows => "Flows",
            Self::Search => "Search",
            Self::Plugins => "Plugins",
            Self::Processes => "Processes",
            Self::Ports => "Ports",
            Self::Worktrees => "Worktrees",
        }
    }

    pub(crate) fn docked(self) -> bool {
        matches!(self, Self::Changes | Self::Files | Self::Todo | Self::Browser | Self::Flow)
    }

    pub(crate) fn shows(self, session: Option<&Session>) -> bool {
        self != Self::Flow || session.is_some_and(Session::shown_runs_flow)
    }

    pub(crate) fn build(self, window: &mut Window, cx: &mut gpui::App) -> AnyView {
        match self {
            Self::Changes => changes::build(window, cx),
            Self::Files => files::build(window, cx),
            Self::Todo => todo::build(window, cx),
            Self::Browser => browser::build(window, cx),
            Self::Flow => flows::build_dock(window, cx),
            Self::Queue => queue::build(window, cx),
            Self::Issues => issues::build(window, cx),
            Self::Usage => usage::build(window, cx),
            Self::Settings => settings::build(window, cx),
            Self::Accounts => accounts::build(window, cx),
            Self::Flows => flows::build(window, cx),
            Self::Search => search::build(window, cx),
            Self::Plugins => plugins::build(window, cx),
            Self::Processes => processes::build(window, cx),
            Self::Ports => ports::build(window, cx),
            Self::Worktrees => worktrees::build(window, cx),
        }
    }
}

#[cfg(test)]
mod tests;
