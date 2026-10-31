use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString, Styled as _, Svg, px, svg};

use super::file_icons;

macro_rules! asset {
    ($name:literal) => {
        (concat!("icons/", $name, ".svg"), include_bytes!(concat!("../../assets/icons/", $name, ".svg")))
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Icon {
    Changes,
    Files,
    Todo,
    Queue,
    Issues,
    Usage,
    Search,
    Flows,
    Accounts,
    Plugins,
    Settings,
    Plus,
    SplitRight,
    SplitDown,
    Agent,
    Keyboard,
    Server,
    Laptop,
    Play,
    Pause,
    Sound,
    Muted,
    Star,
    StarFilled,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Swap,
    Copy,
    CopyPlus,
    Pencil,
    Trash,
    Undo,
    PullRequest,
    Commit,
    Branch,
    Eye,
    EyeOff,
    FolderOpen,
    File,
    CollapseAll,
    Repeat,
    Stop,
    Terminal,
    Download,
    Waypoints,
    Message,
    ChevronRight,
    ChevronDown,
    Alert,
    Processes,
    Ports,
    Worktrees,
    External,
    Help,
    Loader,
    Refresh,
    Check,
    Closed,
    Working,
    Waiting,
    Done,
    Failed,
    Pending,
    Idle,
    Globe,
    Pick,
    Camera,
    Phone,
    Tablet,
    Monitor,
    Close,
    Console,
    Bug,
    Send,
    Devices,
    Rotate,
    Reload,
    Interface,
    Share,
    Key,
}

impl Icon {
    const ALL: [Icon; 82] = [
        Icon::Changes,
        Icon::Files,
        Icon::Todo,
        Icon::Queue,
        Icon::Issues,
        Icon::Usage,
        Icon::Search,
        Icon::Flows,
        Icon::Accounts,
        Icon::Plugins,
        Icon::Settings,
        Icon::Plus,
        Icon::SplitRight,
        Icon::SplitDown,
        Icon::Agent,
        Icon::Keyboard,
        Icon::Server,
        Icon::Laptop,
        Icon::Play,
        Icon::Pause,
        Icon::Sound,
        Icon::Muted,
        Icon::Star,
        Icon::StarFilled,
        Icon::ArrowUp,
        Icon::ArrowDown,
        Icon::ArrowLeft,
        Icon::ArrowRight,
        Icon::Swap,
        Icon::Copy,
        Icon::CopyPlus,
        Icon::Pencil,
        Icon::Trash,
        Icon::Undo,
        Icon::PullRequest,
        Icon::Commit,
        Icon::Branch,
        Icon::Eye,
        Icon::EyeOff,
        Icon::FolderOpen,
        Icon::File,
        Icon::CollapseAll,
        Icon::Repeat,
        Icon::Stop,
        Icon::Terminal,
        Icon::Download,
        Icon::Waypoints,
        Icon::Message,
        Icon::ChevronRight,
        Icon::ChevronDown,
        Icon::Alert,
        Icon::Processes,
        Icon::Ports,
        Icon::Worktrees,
        Icon::External,
        Icon::Help,
        Icon::Loader,
        Icon::Refresh,
        Icon::Check,
        Icon::Closed,
        Icon::Working,
        Icon::Waiting,
        Icon::Done,
        Icon::Failed,
        Icon::Pending,
        Icon::Idle,
        Icon::Globe,
        Icon::Pick,
        Icon::Camera,
        Icon::Phone,
        Icon::Tablet,
        Icon::Monitor,
        Icon::Close,
        Icon::Console,
        Icon::Bug,
        Icon::Send,
        Icon::Devices,
        Icon::Rotate,
        Icon::Reload,
        Icon::Interface,
        Icon::Share,
        Icon::Key,
    ];

    fn asset(self) -> (&'static str, &'static [u8]) {
        self.chrome()
            .or_else(|| self.action())
            .or_else(|| self.status())
            .or_else(|| self.browser())
            .unwrap_or_else(|| self.plan())
    }

    fn chrome(self) -> Option<(&'static str, &'static [u8])> {
        Some(match self {
            Icon::Changes => asset!("git-compare"),
            Icon::Files => asset!("folder-tree"),
            Icon::Todo => asset!("list-checks"),
            Icon::Queue => asset!("calendar-clock"),
            Icon::Issues => asset!("circle-dot"),
            Icon::Usage => asset!("gauge"),
            Icon::Search => asset!("search"),
            Icon::Flows => asset!("workflow"),
            Icon::Accounts => asset!("users"),
            Icon::Plugins => asset!("puzzle"),
            Icon::Settings => asset!("settings"),
            Icon::Plus => asset!("plus"),
            Icon::SplitRight => asset!("columns-2"),
            Icon::SplitDown => asset!("rows-2"),
            Icon::Agent => asset!("bot"),
            Icon::Keyboard => asset!("keyboard"),
            Icon::Help => asset!("circle-help"),
            Icon::Server => asset!("server"),
            Icon::Laptop => asset!("laptop"),
            Icon::Processes => asset!("activity"),
            Icon::Ports => asset!("plug"),
            Icon::Worktrees => asset!("hard-drive"),
            Icon::Globe => asset!("globe"),
            Icon::Interface => asset!("app-window"),
            Icon::Share => asset!("share-2"),
            _ => return None,
        })
    }

    fn action(self) -> Option<(&'static str, &'static [u8])> {
        Some(match self {
            Icon::Play => asset!("play-filled"),
            Icon::Pause => asset!("pause"),
            Icon::Sound => asset!("volume-2"),
            Icon::Muted => asset!("volume-x"),
            Icon::Star => asset!("star"),
            Icon::StarFilled => asset!("star-filled"),
            Icon::ArrowUp => asset!("arrow-up"),
            Icon::ArrowDown => asset!("arrow-down"),
            Icon::ArrowLeft => asset!("arrow-left"),
            Icon::ArrowRight => asset!("arrow-right"),
            Icon::Swap => asset!("arrow-left-right"),
            Icon::Copy => asset!("copy"),
            Icon::CopyPlus => asset!("copy-plus"),
            Icon::Pencil => asset!("pencil"),
            Icon::Trash => asset!("trash-2"),
            Icon::Undo => asset!("rotate-ccw"),
            Icon::Stop => asset!("circle-stop"),
            Icon::Terminal => asset!("square-terminal"),
            Icon::Download => asset!("download"),
            Icon::ChevronRight => asset!("chevron-right"),
            Icon::ChevronDown => asset!("chevron-down"),
            Icon::CollapseAll => asset!("chevrons-down-up"),
            Icon::Alert => asset!("triangle-alert"),
            Icon::External => asset!("external-link"),
            Icon::Loader => asset!("loader-circle"),
            Icon::Refresh => asset!("refresh-cw"),
            Icon::Check => asset!("check"),
            Icon::Closed => asset!("circle-check"),
            Icon::Key => asset!("key-round"),
            _ => return None,
        })
    }

    fn status(self) -> Option<(&'static str, &'static [u8])> {
        Some(match self {
            Icon::Working => asset!("status-working"),
            Icon::Waiting => asset!("status-waiting"),
            Icon::Done => asset!("status-done"),
            Icon::Failed => asset!("status-failed"),
            Icon::Pending => asset!("status-pending"),
            Icon::Idle => asset!("status-idle"),
            _ => return None,
        })
    }

    fn browser(self) -> Option<(&'static str, &'static [u8])> {
        Some(match self {
            Icon::Pick => asset!("mouse-pointer-click"),
            Icon::Camera => asset!("camera"),
            Icon::Phone => asset!("smartphone"),
            Icon::Tablet => asset!("tablet"),
            Icon::Monitor => asset!("monitor"),
            Icon::Close => asset!("x"),
            Icon::Console => asset!("panel-bottom"),
            Icon::Bug => asset!("bug"),
            Icon::Send => asset!("send"),
            Icon::Devices => asset!("monitor-smartphone"),
            Icon::Rotate => asset!("rotate-cw-square"),
            Icon::Reload => asset!("rotate-cw"),
            _ => return None,
        })
    }

    fn plan(self) -> (&'static str, &'static [u8]) {
        match self {
            Icon::PullRequest => asset!("git-pull-request"),
            Icon::Commit => asset!("git-commit-horizontal"),
            Icon::Branch => asset!("git-branch"),
            Icon::Eye => asset!("eye"),
            Icon::EyeOff => asset!("eye-off"),
            Icon::FolderOpen => asset!("folder-open"),
            Icon::File => asset!("file-text"),
            Icon::Repeat => asset!("repeat"),
            Icon::Waypoints => asset!("waypoints"),
            _ => asset!("message-square"),
        }
    }

    pub(crate) fn path(self) -> &'static str {
        self.asset().0
    }
}

pub(crate) struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let icon = Icon::ALL.into_iter().map(Icon::asset).find(|(p, _)| *p == path).map(|(_, bytes)| bytes);
        Ok(icon.or_else(|| file_icons::load(path)).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let every = Icon::ALL.into_iter().map(Icon::path).chain(file_icons::paths());
        Ok(every.filter(|p| p.starts_with(path)).map(SharedString::from).collect())
    }
}

pub(crate) fn icon(icon: Icon, size: f32) -> Svg {
    svg().path(icon.path()).size(px(size)).flex_none()
}

#[cfg(test)]
mod tests;
