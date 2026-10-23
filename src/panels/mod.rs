//! The panels: docked beside the terminal (changes, files, TODO) or shown as a sheet over the
//! window (issues, usage, settings…). Each lives in its own module and builds its own view; the
//! root view hosts them and keeps each one alive once opened, so it keeps its state.

pub(crate) mod accounts;
pub(crate) mod changes;
pub(crate) mod files;
pub(crate) mod flows;
pub(crate) mod issues;
pub(crate) mod plugins;
pub(crate) mod queue;
pub(crate) mod search;
pub(crate) mod settings;
pub(crate) mod todo;
pub(crate) mod usage;

use gpui::{AnyView, Window};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PanelKind {
    Changes,
    Files,
    Todo,
    Queue,
    Issues,
    Usage,
    Settings,
    Accounts,
    Flows,
    Search,
    Plugins,
}

impl PanelKind {
    pub(crate) const ALL: [PanelKind; 11] = [
        PanelKind::Changes,
        PanelKind::Files,
        PanelKind::Todo,
        PanelKind::Queue,
        PanelKind::Issues,
        PanelKind::Usage,
        PanelKind::Settings,
        PanelKind::Accounts,
        PanelKind::Flows,
        PanelKind::Search,
        PanelKind::Plugins,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Changes => "Changes",
            Self::Files => "Files",
            Self::Todo => "TODO",
            Self::Queue => "Queue",
            Self::Issues => "Issues",
            Self::Usage => "Usage",
            Self::Settings => "Settings",
            Self::Accounts => "Accounts",
            Self::Flows => "Flows",
            Self::Search => "Search",
            Self::Plugins => "Plugins",
        }
    }

    /// Docked beside the terminal, rather than a sheet over the window.
    pub(crate) fn docked(self) -> bool {
        matches!(self, Self::Changes | Self::Files | Self::Todo)
    }

    pub(crate) fn build(self, window: &mut Window, cx: &mut gpui::App) -> AnyView {
        match self {
            Self::Changes => changes::build(window, cx),
            Self::Files => files::build(window, cx),
            Self::Todo => todo::build(window, cx),
            Self::Queue => queue::build(window, cx),
            Self::Issues => issues::build(window, cx),
            Self::Usage => usage::build(window, cx),
            Self::Settings => settings::build(window, cx),
            Self::Accounts => accounts::build(window, cx),
            Self::Flows => flows::build(window, cx),
            Self::Search => search::build(window, cx),
            Self::Plugins => plugins::build(window, cx),
        }
    }
}
