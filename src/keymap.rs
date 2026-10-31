use crate::panels::PanelKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    Panel(PanelKind),
    FindFile,
    Copy,
    Paste,
    NewTab,
    NewWorkspace,
    StartAgent,
    OpenProject,
    Connect,
    SplitRight,
    SplitDown,
    Close,
    CloseTab,
    PreviousTab,
    NextTab,
    Tab,
    PreviousPane,
    NextPane,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Shortcuts,
    Back,
    Forward,
    Reload,
    FocusAddress,
    Cut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Group {
    Panels,
    Window,
    TabsAndPanes,
    Terminal,
    Browser,
}

pub(crate) struct Shortcut {
    pub keys: &'static [&'static str],
    pub shift: bool,
    pub command: Command,
}

const fn key(keys: &'static [&'static str], shift: bool, command: Command) -> Shortcut {
    Shortcut { keys, shift, command }
}

const DIGITS: &[&str] = &["1", "2", "3", "4", "5", "6", "7", "8", "9"];

pub(crate) const SHORTCUTS: &[Shortcut] = &[
    key(&["c"], true, Command::Panel(PanelKind::Changes)),
    key(&["e"], true, Command::Panel(PanelKind::Files)),
    key(&["p"], true, Command::FindFile),
    key(&["l"], true, Command::Panel(PanelKind::Todo)),
    key(&["b"], true, Command::Panel(PanelKind::Browser)),
    key(&["f"], false, Command::Panel(PanelKind::Search)),
    key(&["i"], false, Command::Panel(PanelKind::Issues)),
    key(&["s"], true, Command::Panel(PanelKind::Queue)),
    key(&["g"], true, Command::Panel(PanelKind::Flows)),
    key(&["a"], true, Command::Panel(PanelKind::Accounts)),
    key(&["u"], false, Command::Panel(PanelKind::Usage)),
    key(&["x"], true, Command::Panel(PanelKind::Plugins)),
    key(&["m"], true, Command::Panel(PanelKind::Processes)),
    key(&["o"], true, Command::Panel(PanelKind::Ports)),
    key(&["t"], true, Command::Panel(PanelKind::Worktrees)),
    key(&[","], false, Command::Panel(PanelKind::Settings)),
    key(&["n"], false, Command::NewWorkspace),
    key(&["o"], false, Command::OpenProject),
    key(&["k"], true, Command::Connect),
    key(&["g"], false, Command::StartAgent),
    key(&["w"], false, Command::Close),
    key(&["/", "?"], false, Command::Shortcuts),
    key(&["t"], false, Command::NewTab),
    key(&["w"], true, Command::CloseTab),
    key(DIGITS, false, Command::Tab),
    key(&["["], true, Command::PreviousTab),
    key(&["]"], true, Command::NextTab),
    key(&["d"], false, Command::SplitRight),
    key(&["d"], true, Command::SplitDown),
    key(&["["], false, Command::PreviousPane),
    key(&["]"], false, Command::NextPane),
    key(&["c"], false, Command::Copy),
    key(&["v"], false, Command::Paste),
    key(&["+", "="], false, Command::ZoomIn),
    key(&["-"], false, Command::ZoomOut),
    key(&["0"], false, Command::ZoomReset),
    key(&["l"], false, Command::FocusAddress),
    key(&["r"], false, Command::Reload),
    key(&["["], false, Command::Back),
    key(&["]"], false, Command::Forward),
    key(&["x"], false, Command::Cut),
];

pub(crate) const SYSTEM_KEYS: &[(&str, &str)] = &[
    ("⌘H", "Hide cornercase"),
    ("⌥⌘H", "Hide the other apps"),
    ("⌘M", "Minimise the window"),
    ("⌃⌘F", "Full screen"),
    ("⌘Q", "Quit"),
];

pub(crate) const OTHER_KEYS: &[(&str, &str)] = &[
    ("Esc", "Close what is on top, or back to the shell; a page in the browser gets it"),
    ("Tab", "Next field of a dialog, ⇧Tab back"),
    ("↑ ↓ ↩", "Choose in a menu"),
    ("⌘-click", "Open a link or a path"),
];

impl Shortcut {
    pub(crate) fn label(&self) -> String {
        match self.command {
            Command::Tab => "⌘1…9".to_string(),
            _ => keys_label(self.keys[0], self.shift),
        }
    }

    fn takes(&self, key: &str) -> bool {
        self.keys.contains(&key)
    }

    fn in_browser(&self) -> bool {
        self.command.group() == Group::Browser
    }
}

impl Command {
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::Panel(kind) => kind.title(),
            Self::FindFile => "Go to a file",
            Self::Copy => "Copy the selection",
            Self::Paste => "Paste",
            Self::NewTab => "New tab",
            Self::NewWorkspace => "New workspace",
            Self::StartAgent => "Start an agent",
            Self::OpenProject => "Open a project",
            Self::Connect => "Connect to another server",
            Self::SplitRight => "Split right",
            Self::SplitDown => "Split down",
            _ => self.describe_more(),
        }
    }

    fn describe_more(self) -> &'static str {
        match self {
            Self::Close => "Close what is on top, else the pane",
            Self::CloseTab => "Close the tab",
            Self::PreviousTab => "Previous tab",
            Self::NextTab => "Next tab",
            Self::Tab => "Go to a tab (⌘9 the last)",
            Self::PreviousPane => "Previous pane",
            Self::NextPane => "Next pane",
            Self::ZoomIn => "Bigger text",
            Self::ZoomOut => "Smaller text",
            Self::ZoomReset => "Text back to its size",
            _ => self.describe_browser(),
        }
    }

    fn describe_browser(self) -> &'static str {
        match self {
            Self::Back => "Back",
            Self::Forward => "Forward",
            Self::Reload => "Reload the page",
            Self::FocusAddress => "Go to the address",
            Self::Cut => "Cut the selection",
            _ => "Keyboard shortcuts",
        }
    }

    pub(crate) fn group(self) -> Group {
        match self {
            Self::Panel(_) | Self::FindFile => Group::Panels,
            Self::NewTab
            | Self::CloseTab
            | Self::PreviousTab
            | Self::NextTab
            | Self::Tab
            | Self::SplitRight
            | Self::SplitDown
            | Self::PreviousPane
            | Self::NextPane => Group::TabsAndPanes,
            Self::Copy | Self::Paste | Self::ZoomIn | Self::ZoomOut | Self::ZoomReset => Group::Terminal,
            Self::Back | Self::Forward | Self::Reload | Self::FocusAddress | Self::Cut => Group::Browser,
            _ => Group::Window,
        }
    }
}

impl Group {
    #[cfg(test)]
    pub(crate) const ALL: [Group; 5] =
        [Group::Panels, Group::Window, Group::TabsAndPanes, Group::Terminal, Group::Browser];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Panels => "Panels",
            Self::Window => "Window",
            Self::TabsAndPanes => "Tabs and panes",
            Self::Terminal => "Terminal",
            Self::Browser => "Browser",
        }
    }
}

pub(crate) fn find(key: &str, shift: bool, browser: bool) -> Option<&'static Shortcut> {
    SHORTCUTS
        .iter()
        .filter(|s| s.takes(key) && (s.shift == shift || shift) && (browser || !s.in_browser()))
        .min_by_key(|s| (s.shift != shift, !s.in_browser()))
}

pub(crate) fn in_group(group: Group) -> impl Iterator<Item = &'static Shortcut> {
    SHORTCUTS.iter().filter(move |s| s.command.group() == group)
}

pub(crate) fn keys_of(command: Command) -> Option<String> {
    SHORTCUTS.iter().find(|s| s.command == command).map(Shortcut::label)
}

pub(crate) fn keys_label(key: &str, shift: bool) -> String {
    format!("{}⌘{}", if shift { "⇧" } else { "" }, key.to_uppercase())
}

pub(crate) fn with_keys(label: &str, command: Command) -> String {
    keys_of(command).map_or_else(|| label.to_string(), |keys| format!("{label}  {keys}"))
}

pub(crate) fn tab_index(key: &str, tabs: usize) -> Option<usize> {
    let at = DIGITS.iter().position(|d| *d == key)?;
    if at == DIGITS.len() - 1 { tabs.checked_sub(1) } else { (at < tabs).then_some(at) }
}

#[cfg(test)]
mod tests;
