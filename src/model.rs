//! The JSON cornercase prints: session snapshots (`cornercase view`) and terminal frames
//! (`cornercase surface --patches`), and the grid the frames and patches build up.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use serde::Deserialize;

// ---------------------------------------------------------------------------------------------
// The session, as `cornercase view` prints it on every change.

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Session {
    pub boot: String,
    pub revision: u64,
    pub version: String,
    pub groups: Vec<Group>,
    pub projects: Vec<Project>,
    pub view: Option<ViewInfo>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Group {
    pub id: u64,
    pub name: String,
    pub collapsed: bool,
    pub icon: Option<String>,
    pub colour: Option<u8>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Project {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub group: Option<u64>,
    pub active: bool,
    pub collapsed: bool,
    pub workspaces: Vec<Workspace>,
    /// The name given in cornercase, if any; `name` falls back to the folder's.
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Workspace {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    pub worktree: bool,
    pub active: bool,
    pub collapsed: bool,
    pub behind: u64,
    pub tabs: Vec<Tab>,
    pub custom_name: Option<String>,
    /// The pull request of its branch, with its checks.
    pub pull_request: Option<PullRequest>,
    /// The project's setup in this workspace: `running` or `failed`.
    pub setup: Option<String>,
    /// What changed, as the changes panel counts it.
    pub changes: Option<Changes>,
    /// Agents closed with `agents close`, waiting for `agents reopen`.
    pub closed_agents: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    /// `open`, `draft`, `merged` or `closed`.
    pub state: String,
    /// The rollup of its checks: `failing`, `pending` or `passing`; none without checks.
    pub checks: Option<String>,
    pub failed: Vec<String>,
    pub conflicts: bool,
    pub base: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Changes {
    pub added: u64,
    pub removed: u64,
    pub files: u64,
    pub mode: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Tab {
    pub id: u64,
    pub name: String,
    pub status: Option<String>,
    pub active: bool,
    pub panes: Vec<PaneInfo>,
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PaneInfo {
    pub id: u64,
    pub path: String,
    pub program: Option<String>,
    pub agent: Option<String>,
    pub status: Option<String>,
    pub model: Option<String>,
    pub context: Option<u32>,
    pub active: bool,
    pub title: Option<String>,
    pub dialog: bool,
    /// The agent that started this one, for the agents tree.
    pub parent: Option<u64>,
    /// Subagents it runs inside itself (Claude Code's tasks).
    pub subagents: u32,
    /// Its role in a flow run.
    pub role: Option<String>,
    /// Whether the agents it started are folded away.
    pub collapsed: bool,
    pub resuming: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ViewInfo {
    pub id: u64,
    pub projection: u64,
    pub active: bool,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub shown: Shown,
    pub controls: bool,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct Shown {
    pub project: Option<u64>,
    pub workspace: Option<u64>,
    pub tab: Option<u64>,
    pub pane: Option<u64>,
}

impl Session {
    pub fn shown(&self) -> Shown {
        self.view.as_ref().map(|v| v.shown).unwrap_or_default()
    }

    pub fn workspace(&self, id: u64) -> Option<(&Project, &Workspace)> {
        self.projects
            .iter()
            .find_map(|p| p.workspaces.iter().find(|w| w.id == id).map(|w| (p, w)))
    }

    pub fn pane(&self, id: u64) -> Option<&PaneInfo> {
        self.projects
            .iter()
            .flat_map(|p| &p.workspaces)
            .flat_map(|w| &w.tabs)
            .flat_map(|t| &t.panes)
            .find(|p| p.id == id)
    }
}

/// How much an agent's state asks for attention, to show the loudest one of a tab or workspace.
pub fn status_rank(status: Option<&str>) -> u8 {
    match status {
        Some("waiting") => 5,
        Some("failed") => 4,
        Some("done") => 3,
        Some("pending") => 2,
        Some("working") => 1,
        _ => 0,
    }
}

impl Tab {
    pub fn loudest(&self) -> Option<&str> {
        let tab = self.status.as_deref();
        let panes = self.panes.iter().filter_map(|p| p.status.as_deref());
        std::iter::once(tab).flatten().chain(panes).max_by_key(|s| status_rank(Some(s)))
    }

    pub fn agent_pane(&self) -> Option<&PaneInfo> {
        self.panes.iter().find(|p| p.agent.is_some())
    }
}

impl Workspace {
    pub fn loudest(&self) -> Option<&str> {
        self.tabs.iter().filter_map(Tab::loudest).max_by_key(|s| status_rank(Some(s)))
    }
}

/// An agent's pane as the sidebar lists it under its workspace: in a tree of who started whom.
#[derive(Debug, Clone, Copy)]
pub struct AgentRow<'a> {
    pub pane: &'a PaneInfo,
    pub tab: u64,
    pub depth: usize,
    /// How many agents it started in this workspace, shown or folded away.
    pub children: usize,
}

impl Workspace {
    /// Its agents in tab order, each followed by the agents it started (a parent outside the
    /// workspace counts as none), leaving out those under a folded agent.
    pub fn agent_rows(&self) -> Vec<AgentRow<'_>> {
        let agents: Vec<(u64, &PaneInfo)> = self
            .tabs
            .iter()
            .flat_map(|t| t.panes.iter().filter(|p| p.agent.is_some()).map(move |p| (t.id, p)))
            .collect();
        let known = |id: Option<u64>| id.filter(|id| agents.iter().any(|(_, p)| p.id == *id));
        let children = |id: u64| agents.iter().filter(|(_, p)| known(p.parent) == Some(id)).count();
        let mut rows = Vec::new();
        // Depth first, with a stack and a seen set so a cycle in the data cannot loop forever.
        let mut seen = std::collections::HashSet::new();
        let mut stack: Vec<(u64, &PaneInfo, usize)> =
            agents.iter().rev().filter(|(_, p)| known(p.parent).is_none()).map(|(t, p)| (*t, *p, 0)).collect();
        while let Some((tab, pane, depth)) = stack.pop() {
            if !seen.insert(pane.id) {
                continue;
            }
            rows.push(AgentRow { pane, tab, depth, children: children(pane.id) });
            if pane.collapsed {
                continue;
            }
            for (t, p) in agents.iter().rev().filter(|(_, p)| known(p.parent) == Some(pane.id)) {
                stack.push((*t, p, depth + 1));
            }
        }
        rows
    }
}

impl Project {
    pub fn loudest(&self) -> Option<&str> {
        self.workspaces.iter().filter_map(Workspace::loudest).max_by_key(|s| status_rank(Some(s)))
    }
}

// ---------------------------------------------------------------------------------------------
// Frames, as `cornercase surface --patches` prints them.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Colour {
    Index(u8),
    Rgb([u8; 3]),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Style {
    pub fg: Option<Colour>,
    pub bg: Option<Colour>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub underline_style: Option<UnderlineKind>,
    pub underline_colour: Option<Colour>,
    pub overline: bool,
    pub blink: bool,
    pub inverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnderlineKind {
    Double,
    Curly,
    Dotted,
    Dashed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Hyperlink {
    pub uri: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Area {
    pub x: u16,
    pub y: u16,
    pub cols: u16,
    pub rows: u16,
}

impl Area {
    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.cols && y < self.y + self.rows
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Divider {
    pub dir: String,
    pub path: Vec<bool>,
    pub area: Area,
    pub line: Area,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorShape {
    #[default]
    Block,
    Bar,
    Underline,
    BlockHollow,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub shape: CursorShape,
    pub blinking: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseMode {
    #[default]
    None,
    Press,
    PressRelease,
    ButtonMotion,
    AnyMotion,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Modes {
    pub mouse: MouseMode,
    pub alternate_screen: bool,
    pub alternate_scroll: bool,
    pub bracketed_paste: bool,
    pub application_cursor: bool,
    pub focus_events: bool,
    pub kitty_keyboard: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Scroll {
    pub total: u64,
    pub offset: u64,
    pub rows: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Colours {
    pub fg: Option<[u8; 3]>,
    pub bg: Option<[u8; 3]>,
    pub cursor: Option<[u8; 3]>,
    pub palette: BTreeMap<String, [u8; 3]>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct WireRun {
    pub style: usize,
    pub hyperlink: Option<usize>,
    pub cells: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum WireLines {
    All(Vec<Vec<WireRun>>),
    Changed(HashMap<String, Vec<WireRun>>),
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Copy {
    pub from: u16,
    pub to: u16,
    pub count: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WirePane {
    pub pane: u64,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub rect: Option<Area>,
    #[serde(default)]
    pub size: Option<Size>,
    #[serde(default)]
    pub cursor: Option<Cursor>,
    #[serde(default)]
    pub modes: Option<Modes>,
    #[serde(default)]
    pub scroll: Option<Scroll>,
    #[serde(default)]
    pub colours: Option<Colours>,
    #[serde(default)]
    pub copy: Option<Copy>,
    #[serde(default)]
    pub lines: Option<WireLines>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Frame {
    pub boot: String,
    pub revision: u64,
    pub surface: u64,
    pub tab: u64,
    pub cols: u16,
    pub rows: u16,
    pub base: Option<u64>,
    pub projection: Option<u64>,
    pub dividers: Vec<Divider>,
    pub styles: Vec<Style>,
    pub hyperlinks: Vec<Hyperlink>,
    pub panes: Vec<WirePane>,
}

// ---------------------------------------------------------------------------------------------
// The grid the client keeps: styles and links resolved, so patches with their own tables apply.

#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub style: Style,
    pub link: Option<Arc<str>>,
    pub cells: Vec<String>,
}

pub type Line = Vec<Run>;

#[derive(Debug, Clone, Default)]
pub struct PaneGrid {
    pub pane: u64,
    pub active: bool,
    pub rect: Area,
    pub size: Size,
    pub cursor: Option<Cursor>,
    pub modes: Modes,
    pub scroll: Scroll,
    pub colours: Colours,
    pub lines: Vec<Line>,
}

impl PaneGrid {
    /// The cell at a column and row of the pane's terminal: its text, style and link.
    pub fn cell(&self, x: u16, y: u16) -> Option<(&str, &Run)> {
        let line = self.lines.get(y as usize)?;
        let mut col = 0usize;
        for run in line {
            if (x as usize) < col + run.cells.len() {
                return Some((&run.cells[x as usize - col], run));
            }
            col += run.cells.len();
        }
        None
    }
}

#[derive(Debug, Clone, Default)]
pub struct Grid {
    pub boot: String,
    pub surface: u64,
    pub revision: u64,
    pub tab: u64,
    pub cols: u16,
    pub rows: u16,
    pub dividers: Vec<Divider>,
    pub panes: Vec<PaneGrid>,
}

pub enum Applied {
    Ok,
    /// A patch whose base we do not hold: ask for a complete frame (`cornercase redraw`).
    NeedRedraw,
}

fn resolve(runs: Vec<WireRun>, styles: &[Style], links: &[Arc<str>]) -> Line {
    runs.into_iter()
        .map(|r| Run {
            style: styles.get(r.style).copied().unwrap_or_default(),
            link: r.hyperlink.and_then(|h| links.get(h).cloned()),
            cells: r.cells,
        })
        .collect()
}

impl Grid {
    pub fn active_pane(&self) -> Option<&PaneGrid> {
        self.panes.iter().find(|p| p.active).or(self.panes.first())
    }

    pub fn pane(&self, id: u64) -> Option<&PaneGrid> {
        self.panes.iter().find(|p| p.pane == id)
    }

    /// The pane under a cell of the surface.
    pub fn pane_at(&self, x: u16, y: u16) -> Option<&PaneGrid> {
        self.panes.iter().find(|p| p.rect.contains(x, y))
    }

    pub fn apply(&mut self, frame: Frame) -> Applied {
        let styles = frame.styles;
        let links: Vec<Arc<str>> = frame.hyperlinks.into_iter().map(|h| Arc::from(h.uri)).collect();
        match frame.base {
            None => {
                self.boot = frame.boot;
                self.surface = frame.surface;
                self.revision = frame.revision;
                self.tab = frame.tab;
                self.cols = frame.cols;
                self.rows = frame.rows;
                self.dividers = frame.dividers;
                self.panes = frame
                    .panes
                    .into_iter()
                    .map(|p| PaneGrid {
                        pane: p.pane,
                        active: p.active.unwrap_or(false),
                        rect: p.rect.unwrap_or_default(),
                        size: p.size.unwrap_or_default(),
                        cursor: p.cursor,
                        modes: p.modes.unwrap_or_default(),
                        scroll: p.scroll.unwrap_or_default(),
                        colours: p.colours.unwrap_or_default(),
                        lines: match p.lines {
                            Some(WireLines::All(lines)) => {
                                lines.into_iter().map(|l| resolve(l, &styles, &links)).collect()
                            }
                            _ => Vec::new(),
                        },
                    })
                    .collect();
                Applied::Ok
            }
            Some(base) => {
                if base != self.revision || frame.surface != self.surface || frame.boot != self.boot {
                    return Applied::NeedRedraw;
                }
                for p in frame.panes {
                    let Some(grid) = self.panes.iter_mut().find(|g| g.pane == p.pane) else {
                        return Applied::NeedRedraw;
                    };
                    grid.cursor = p.cursor;
                    if let Some(active) = p.active {
                        grid.active = active;
                    }
                    if let Some(modes) = p.modes {
                        grid.modes = modes;
                    }
                    if let Some(scroll) = p.scroll {
                        grid.scroll = scroll;
                    }
                    if let Some(colours) = p.colours {
                        grid.colours = colours;
                    }
                    if let Some(copy) = p.copy {
                        let held = grid.lines.clone();
                        for i in 0..copy.count as usize {
                            let (from, to) = (copy.from as usize + i, copy.to as usize + i);
                            if let (Some(line), true) = (held.get(from), to < grid.lines.len()) {
                                grid.lines[to] = line.clone();
                            }
                        }
                    }
                    match p.lines {
                        Some(WireLines::Changed(changed)) => {
                            for (row, runs) in changed {
                                let Ok(row) = row.parse::<usize>() else { continue };
                                if row >= grid.lines.len() {
                                    grid.lines.resize(row + 1, Vec::new());
                                }
                                grid.lines[row] = resolve(runs, &styles, &links);
                            }
                        }
                        Some(WireLines::All(lines)) => {
                            grid.lines = lines.into_iter().map(|l| resolve(l, &styles, &links)).collect();
                        }
                        None => {}
                    }
                }
                self.revision = frame.revision;
                Applied::Ok
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_workspace_details_from_the_session() {
        let text = r#"{"projects":[{"id":1,"name":"shop","custom_name":"shop","workspaces":[{"id":2,"name":"feat",
            "behind":3,"setup":"running","changes":{"added":4,"removed":1,"files":2,"mode":"uncommitted"},
            "pull_request":{"number":485,"title":"Dark mode","url":"https://x/485","state":"open","checks":"failing",
            "failed":["test"],"conflicts":false,"base":"main","provider":"github","stack":null},
            "tabs":[{"id":3,"panes":[{"id":4,"agent":"claude","subagents":2,"role":"coder","parent":null}]}]}]}]}"#;
        let session: Session = serde_json::from_str(text).unwrap();
        let ws = &session.projects[0].workspaces[0];
        let pr = ws.pull_request.as_ref().unwrap();
        assert_eq!((pr.number, pr.checks.as_deref(), pr.failed.len()), (485, Some("failing"), 1));
        assert_eq!(ws.setup.as_deref(), Some("running"));
        assert_eq!(ws.changes.as_ref().unwrap().added, 4);
        assert_eq!(ws.tabs[0].panes[0].subagents, 2);
        assert_eq!(session.projects[0].custom_name.as_deref(), Some("shop"));
    }

    #[test]
    fn agents_form_a_tree_and_folds_hide_children() {
        let pane = |id: u64, parent: Option<u64>| PaneInfo { id, agent: Some("claude".into()), parent, ..Default::default() };
        let shell = PaneInfo { id: 9, ..Default::default() };
        let ws = Workspace {
            tabs: vec![
                Tab { id: 1, panes: vec![pane(10, None), shell], ..Default::default() },
                Tab { id: 2, panes: vec![pane(11, Some(10))], ..Default::default() },
                Tab { id: 3, panes: vec![pane(12, Some(11))], ..Default::default() },
                // A parent in another workspace counts as none.
                Tab { id: 4, panes: vec![pane(13, Some(99))], ..Default::default() },
            ],
            ..Default::default()
        };
        let rows: Vec<(u64, usize, usize)> = ws.agent_rows().iter().map(|r| (r.pane.id, r.depth, r.children)).collect();
        assert_eq!(rows, [(10, 0, 1), (11, 1, 1), (12, 2, 0), (13, 0, 0)]);

        let mut folded = ws.clone();
        folded.tabs[1].panes[0].collapsed = true;
        let rows: Vec<u64> = folded.agent_rows().iter().map(|r| r.pane.id).collect();
        assert_eq!(rows, [10, 11, 13]);
    }

    #[test]
    fn reads_the_fixture_frame() {
        let text = include_str!("../tests/frame.json");
        let frame: Frame = serde_json::from_str(text).unwrap();
        let mut grid = Grid::default();
        assert!(matches!(grid.apply(frame), Applied::Ok));
        assert_eq!(grid.panes.len(), 2);
        let pane = &grid.panes[0];
        assert_eq!(pane.cell(1, 1).unwrap().0, "k");
        assert!(pane.cell(1, 1).unwrap().1.style.bold);
        assert_eq!(pane.cell(3, 1).unwrap().0, "中");
        assert_eq!(pane.cell(2, 2).unwrap().1.link.as_deref(), None);
        assert_eq!(pane.cell(3, 2).unwrap().1.link.as_deref(), Some("https://example.com/docs/a-long-page"));
        assert_eq!(pane.cursor.unwrap().shape, CursorShape::Bar);
        assert_eq!(pane.colours.palette.get("1"), Some(&[255, 0, 0]));
    }

    #[test]
    fn applies_a_patch_with_a_copy() {
        let full = r#"{"boot":"b","revision":1,"surface":3,"tab":1,"cols":4,"rows":3,"styles":[{}],
            "panes":[{"pane":7,"active":true,"rect":{"x":0,"y":0,"cols":4,"rows":3},"size":{"cols":4,"rows":3},
            "cursor":null,"lines":[[{"style":0,"cells":["a"]}],[{"style":0,"cells":["b"]}],[{"style":0,"cells":["c"]}]]}]}"#;
        let patch = r#"{"boot":"b","revision":2,"base":1,"surface":3,"tab":1,"cols":4,"rows":3,
            "styles":[{"bold":true}],"panes":[{"pane":7,"cursor":{"x":1,"y":2},
            "copy":{"from":1,"to":0,"count":2},"lines":{"2":[{"style":0,"cells":["d"]}]}}]}"#;
        let mut grid = Grid::default();
        grid.apply(serde_json::from_str(full).unwrap());
        assert!(matches!(grid.apply(serde_json::from_str(patch).unwrap()), Applied::Ok));
        let pane = &grid.panes[0];
        let text: Vec<&str> = (0..3).map(|y| pane.cell(0, y).unwrap().0).collect();
        assert_eq!(text, ["b", "c", "d"]);
        assert!(pane.cell(0, 2).unwrap().1.style.bold);
        assert_eq!(pane.cursor.unwrap().x, 1);
        assert_eq!(grid.revision, 2);
        let stale = patch.replace("\"revision\":2,\"base\":1", "\"revision\":9,\"base\":5");
        assert!(matches!(grid.apply(serde_json::from_str(&stale).unwrap()), Applied::NeedRedraw));
    }
}
