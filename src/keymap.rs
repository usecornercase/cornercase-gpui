use gpui::{Keystroke, Modifiers, Window};

use crate::panels::PanelKind;
use crate::platform::Os;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    Panel(PanelKind),
    FindFile,
    FindInFile,
    Copy,
    Paste,
    NewTab,
    NewWorkspace,
    StartAgent,
    OpenProject,
    Connect,
    PullRequest,
    SplitRight,
    SplitDown,
    Close,
    CloseTab,
    PreviousTab,
    NextTab,
    Tab,
    PreviousPane,
    NextPane,
    NextWorkspace,
    PreviousWorkspace,
    NextProject,
    PreviousProject,
    AgentNeedingYou,
    Mark,
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
    GettingAround,
    TabsAndPanes,
    Terminal,
    Browser,
    Viewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Focus {
    Elsewhere,
    Browser,
    Viewer,
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
    key(&["h"], true, Command::Panel(PanelKind::Recap)),
    key(&["b"], true, Command::Panel(PanelKind::Browser)),
    key(&["f"], true, Command::Panel(PanelKind::Flow)),
    key(&["f"], false, Command::Panel(PanelKind::Search)),
    key(&["f"], false, Command::FindInFile),
    key(&["i"], false, Command::Panel(PanelKind::Issues)),
    key(&["s"], true, Command::Panel(PanelKind::Queue)),
    key(&["g"], true, Command::Panel(PanelKind::Flows)),
    key(&["a"], true, Command::Panel(PanelKind::Accounts)),
    key(&["n"], true, Command::Panel(PanelKind::Machines)),
    key(&["u"], false, Command::Panel(PanelKind::Usage)),
    key(&["x"], true, Command::Panel(PanelKind::Plugins)),
    key(&["m"], true, Command::Panel(PanelKind::Processes)),
    key(&["o"], true, Command::Panel(PanelKind::Ports)),
    key(&["t"], true, Command::Panel(PanelKind::Worktrees)),
    key(&[","], false, Command::Panel(PanelKind::Settings)),
    key(&["n"], false, Command::NewWorkspace),
    key(&["o"], false, Command::OpenProject),
    key(&["k"], true, Command::Connect),
    key(&["p"], false, Command::PullRequest),
    key(&["g"], false, Command::StartAgent),
    key(&["w"], false, Command::Close),
    key(&["/", "?"], false, Command::Shortcuts),
    key(&["j"], false, Command::NextWorkspace),
    key(&["j"], true, Command::PreviousWorkspace),
    key(&["y"], false, Command::NextProject),
    key(&["y"], true, Command::PreviousProject),
    key(&["b"], false, Command::AgentNeedingYou),
    key(&["r"], false, Command::Mark),
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

const SHIFTED: &[(&str, &str)] = &[
    ("~", "`"),
    ("!", "1"),
    ("@", "2"),
    ("#", "3"),
    ("$", "4"),
    ("%", "5"),
    ("^", "6"),
    ("&", "7"),
    ("*", "8"),
    ("(", "9"),
    (")", "0"),
    ("_", "-"),
    ("+", "="),
    ("{", "["),
    ("}", "]"),
    ("|", "\\"),
    (":", ";"),
    ("\"", "'"),
    ("<", ","),
    (">", "."),
    ("?", "/"),
];

const SHIFTED_ON_LINUX: &[&str] = &["u"];

struct PageKey {
    key: &'static str,
    control: bool,
    alt: bool,
    mac: &'static str,
}

const fn page(key: &'static str, control: bool, alt: bool, mac: &'static str) -> PageKey {
    PageKey { key, control, alt, mac }
}

const PAGE_KEYS: &[PageKey] = &[
    page("c", true, false, "c"),
    page("x", true, false, "x"),
    page("v", true, false, "v"),
    page("l", true, false, "l"),
    page("r", true, false, "r"),
    page("f5", false, false, "r"),
    page("left", false, true, "["),
    page("right", false, true, "]"),
];

const MAC_SYSTEM_KEYS: &[(&str, &str)] = &[
    ("⌘H", "Hide cornercase"),
    ("⌥⌘H", "Hide the other apps"),
    ("⌘M", "Minimise the window"),
    ("⌃⌘F", "Full screen"),
    ("⌘Q", "Quit"),
];

const LINUX_SYSTEM_KEYS: &[(&str, &str)] =
    &[("Ctrl+Shift+M", "Minimise the window"), ("F11", "Full screen"), ("Ctrl+Shift+Q", "Quit")];

const WINDOWS_SYSTEM_KEYS: &[(&str, &str)] =
    &[("Ctrl+Shift+M", "Minimise the window"), ("F11", "Full screen"), ("Alt+F4", "Quit")];

const MAC_OTHER_KEYS: &[(&str, &str)] = &[
    ("Esc", "Close what is on top, or back to the shell; a page in the browser gets it"),
    ("Tab", "Next field of a dialog, ⇧Tab back"),
    ("↑ ↓ ↩", "Choose in a menu"),
    ("Click", "Open a web address as Settings › Links says, a path in Files"),
    ("⌘-click", "Open a link or a path with macOS"),
];

const LINUX_OTHER_KEYS: &[(&str, &str)] = &[
    ("Esc", "Close what is on top, or back to the shell; a page in the browser gets it"),
    ("Tab", "Next field of a dialog, Shift+Tab back"),
    ("↑ ↓ Enter", "Choose in a menu"),
    ("Ctrl+C Ctrl+V", "Copy and paste in a text field or a page"),
    ("Click", "Open a web address as Settings › Links says, a path in Files"),
    ("Ctrl+click", "Open a link or a path with its default app"),
    ("Middle click", "Paste what is selected, Shift+middle click where a program takes the mouse"),
];

const WINDOWS_OTHER_KEYS: &[(&str, &str)] = &[
    ("Esc", "Close what is on top, or back to the shell; a page in the browser gets it"),
    ("Tab", "Next field of a dialog, Shift+Tab back"),
    ("↑ ↓ Enter", "Choose in a menu"),
    ("Ctrl+C Ctrl+V", "Copy and paste in a text field or a page"),
    ("Click", "Open a web address as Settings › Links says, a path in Files"),
    ("Ctrl+click", "Open a link or a path with its default app"),
];

pub(crate) fn system_keys(os: Os) -> &'static [(&'static str, &'static str)] {
    match os {
        Os::Mac => MAC_SYSTEM_KEYS,
        Os::Linux => LINUX_SYSTEM_KEYS,
        Os::Windows => WINDOWS_SYSTEM_KEYS,
    }
}

pub(crate) fn other_keys(os: Os) -> &'static [(&'static str, &'static str)] {
    match os {
        Os::Mac => MAC_OTHER_KEYS,
        Os::Linux => LINUX_OTHER_KEYS,
        Os::Windows => WINDOWS_OTHER_KEYS,
    }
}

pub(crate) fn link_click(os: Os) -> &'static str {
    match os {
        Os::Mac => "⌘-click",
        Os::Linux | Os::Windows => "Ctrl+click",
    }
}

pub(crate) fn opens_with_system(modifiers: Modifiers, os: Os) -> bool {
    match os {
        Os::Mac => modifiers.platform,
        Os::Linux | Os::Windows => modifiers.control,
    }
}

impl Shortcut {
    pub(crate) fn label(&self, os: Os) -> String {
        match (self.command, os) {
            (Command::Tab, Os::Mac) => "⌘1…9".to_string(),
            (Command::Tab, Os::Linux | Os::Windows) => "Alt+1…9".to_string(),
            (_, Os::Linux | Os::Windows) if self.in_browser() => page_label(self.keys[0]),
            _ => keys_label(self.keys[0], self.shift, os),
        }
    }

    fn takes(&self, key: &str) -> bool {
        self.keys.contains(&key)
    }

    fn in_browser(&self) -> bool {
        self.command.group() == Group::Browser
    }

    fn answers_in(&self) -> Focus {
        match self.command.group() {
            Group::Browser => Focus::Browser,
            Group::Viewer => Focus::Viewer,
            _ => Focus::Elsewhere,
        }
    }
}

impl Command {
    pub(crate) fn describe(self, os: Os) -> &'static str {
        match (self, os) {
            (Self::Tab, Os::Linux | Os::Windows) => "Go to a tab (Alt+9 the last)",
            (Self::FindInFile, Os::Mac) => "Find in the file, ↩ ⇧↩ to move",
            (Self::FindInFile, Os::Linux | Os::Windows) => "Find in the file, Enter Shift+Enter",
            _ => self.describe_any(),
        }
    }

    fn describe_any(self) -> &'static str {
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
            Self::PullRequest => "The workspace's pull request",
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
            _ => self.describe_getting_around(),
        }
    }

    fn describe_getting_around(self) -> &'static str {
        match self {
            Self::NextWorkspace => "Next workspace",
            Self::PreviousWorkspace => "Previous workspace",
            Self::NextProject => "Next project",
            Self::PreviousProject => "Previous project",
            Self::AgentNeedingYou => "The agent that needs you",
            Self::Mark => "Mark for review, or as reviewed",
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
            Self::NextWorkspace
            | Self::PreviousWorkspace
            | Self::NextProject
            | Self::PreviousProject
            | Self::AgentNeedingYou
            | Self::Mark => Group::GettingAround,
            Self::Copy | Self::Paste | Self::ZoomIn | Self::ZoomOut | Self::ZoomReset => Group::Terminal,
            Self::Back | Self::Forward | Self::Reload | Self::FocusAddress | Self::Cut => Group::Browser,
            Self::FindInFile => Group::Viewer,
            _ => Group::Window,
        }
    }
}

impl Group {
    #[cfg(test)]
    pub(crate) const ALL: [Group; 7] = [
        Group::Panels,
        Group::Window,
        Group::GettingAround,
        Group::TabsAndPanes,
        Group::Terminal,
        Group::Browser,
        Group::Viewer,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Panels => "Panels",
            Self::Window => "Window",
            Self::GettingAround => "Getting around",
            Self::TabsAndPanes => "Tabs and panes",
            Self::Terminal => "Terminal",
            Self::Browser => "Browser",
            Self::Viewer => "Files viewer",
        }
    }
}

pub(crate) fn find(key: &str, shift: bool, focus: Focus, os: Os) -> Option<&'static Shortcut> {
    let either = shift && os == Os::Mac;
    SHORTCUTS
        .iter()
        .filter(|s| s.takes(key) && (s.shift == shift || either))
        .filter(|s| s.answers_in() == Focus::Elsewhere || s.answers_in() == focus)
        .min_by_key(|s| (s.shift != shift, s.answers_in() != focus))
}

pub(crate) fn typed_with_alt_gr(ks: &Keystroke, os: Os) -> bool {
    let m = ks.modifiers;
    let mut chars = ks.key_char.as_deref().unwrap_or_default().chars();
    let typed = chars.next().filter(|c| !c.is_control()).filter(|_| chars.next().is_none());
    os == Os::Windows
        && m.control
        && m.alt
        && !m.platform
        && typed.is_some_and(|c| !c.to_lowercase().eq(ks.key.to_lowercase().chars()))
}

pub(crate) fn left_to_system(ks: &Keystroke, os: Os) -> bool {
    let m = ks.modifiers;
    os == Os::Windows && ks.key == "f4" && m.alt && !m.control && !m.shift && !m.platform
}

pub(crate) fn shift_held(ks: &Keystroke, window: &Window) -> bool {
    ks.modifiers.shift || window.modifiers().shift
}

pub(crate) fn chord(ks: &Keystroke, shift: bool, os: Os, browser: bool) -> Option<(&str, bool)> {
    match os {
        Os::Mac => ks.modifiers.platform.then_some((ks.key.as_str(), ks.modifiers.shift)),
        Os::Windows if typed_with_alt_gr(ks, os) => None,
        Os::Linux | Os::Windows => control_chord(ks, shift, browser),
    }
}

pub(crate) fn shortcut(ks: &Keystroke, shift: bool, os: Os, browser: bool) -> Option<&'static Shortcut> {
    let (key, shift) = chord(ks, shift, os, browser)?;
    find(key, shift, if browser { Focus::Browser } else { Focus::Elsewhere }, os)
}

fn control_chord(ks: &Keystroke, shift: bool, browser: bool) -> Option<(&str, bool)> {
    let m = ks.modifiers;
    if m.platform || m.function {
        return None;
    }
    if let Some(mac) = page_key(ks).filter(|_| browser) {
        return Some((mac, false));
    }
    if m.alt && !m.control && !m.shift {
        return DIGITS.contains(&ks.key.as_str()).then_some((ks.key.as_str(), false));
    }
    if !m.control || !shift {
        return None;
    }
    let base = unshifted(&ks.key).unwrap_or(ks.key.as_str());
    match (DIGITS.contains(&base), SHIFTED_ON_LINUX.contains(&base), m.alt) {
        (true, ..) | (false, true, false) => None,
        (false, true, true) => Some((base, false)),
        (false, false, alt) => Some((base, alt)),
    }
}

fn unshifted(key: &str) -> Option<&'static str> {
    SHIFTED.iter().find(|(shifted, _)| *shifted == key).map(|(_, base)| *base)
}

fn page_key(ks: &Keystroke) -> Option<&'static str> {
    let m = ks.modifiers;
    PAGE_KEYS.iter().find(|p| p.key == ks.key && p.control == m.control && p.alt == m.alt && !m.shift).map(|p| p.mac)
}

fn page_label(mac: &str) -> String {
    PAGE_KEYS.iter().find(|p| p.mac == mac).map_or_else(
        || keys_label(mac, false, Os::Linux),
        |p| {
            format!(
                "{}{}",
                if p.alt {
                    "Alt+"
                } else if p.control {
                    "Ctrl+"
                } else {
                    ""
                },
                key_name(p.key)
            )
        },
    )
}

fn key_name(key: &str) -> String {
    match key {
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        _ => key.to_uppercase(),
    }
}

pub(crate) fn in_group(group: Group) -> impl Iterator<Item = &'static Shortcut> {
    SHORTCUTS.iter().filter(move |s| s.command.group() == group)
}

pub(crate) fn keys_of(command: Command, os: Os) -> Option<String> {
    SHORTCUTS.iter().find(|s| s.command == command).map(|s| s.label(os))
}

pub(crate) fn keys_label(key: &str, shift: bool, os: Os) -> String {
    match os {
        Os::Mac => format!("{}⌘{}", if shift { "⇧" } else { "" }, key.to_uppercase()),
        Os::Linux | Os::Windows => {
            let base = unshifted(key).unwrap_or(key);
            let shifted = shift || SHIFTED_ON_LINUX.contains(&base);
            format!("{}{}", if shifted { "Ctrl+Alt+Shift+" } else { "Ctrl+Shift+" }, key_name(base))
        }
    }
}

pub(crate) fn with_keys(label: &str, command: Command) -> String {
    labelled(label, command, Os::CURRENT)
}

fn labelled(label: &str, command: Command, os: Os) -> String {
    keys_of(command, os).map_or_else(|| label.to_string(), |keys| format!("{label}  {keys}"))
}

pub(crate) fn tab_index(key: &str, tabs: usize) -> Option<usize> {
    let at = DIGITS.iter().position(|d| *d == key)?;
    if at == DIGITS.len() - 1 { tabs.checked_sub(1) } else { (at < tabs).then_some(at) }
}

#[cfg(test)]
mod tests;
