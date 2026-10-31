use super::data::{
    Found, Heat, Order, PaneUse, Process, Sample, command_line, counts, cpu, folded, found, heading, heat, load,
    memory, ordered, place, sample_command, tree, under,
};
use crate::model::{PaneInfo, Project, Session, Tab, Workspace};
use crate::test_util::fixture;

fn sample() -> Sample {
    serde_json::from_str(fixture!("processes/sample.json")).expect("the sample parses")
}

fn process(pid: u64, parent: Option<u64>, cpu: f64) -> Process {
    Process { pid, parent, cpu: Some(cpu), name: format!("p{pid}"), ..Process::default() }
}

fn pids(rows: &[(usize, &Process)]) -> Vec<u64> {
    rows.iter().map(|(_, p)| p.pid).collect()
}

fn session(panes: usize) -> Session {
    let tab = Tab { id: 24, name: "dev".into(), panes: vec![PaneInfo::default(); panes], ..Tab::default() };
    let workspace = Workspace { id: 22, name: "main".into(), tabs: vec![tab], ..Workspace::default() };
    let project = Project { name: "shop".into(), workspaces: vec![workspace], ..Project::default() };
    Session { projects: vec![project], ..Session::default() }
}

#[test]
fn a_sample_reads_its_panes_and_cornercase_itself() {
    let s = sample();
    assert_eq!(s.panes.len(), 2);
    assert_eq!(s.panes[1].pane, 28);
    let own = s.cornercase.as_ref().expect("cornercase's own line");
    assert_eq!((own.pid, own.processes.len()), (28257, 1));
}

#[test]
fn the_tree_gives_each_process_its_depth() {
    let s = sample();
    let depths: Vec<usize> = tree(&s.panes[1].processes).into_iter().map(|(d, _)| d).collect();
    assert_eq!(depths, [0, 1, 2]);
}

#[test]
fn a_process_whose_parent_is_not_listed_starts_a_tree() {
    let orphan = Process { pid: 9, parent: Some(1), ..Process::default() };
    assert_eq!(tree(std::slice::from_ref(&orphan))[0].0, 0);
}

#[test]
fn the_pane_row_stands_for_its_shell() {
    let s = sample();
    let pane = &s.panes[1];
    let rows = under(&pane.processes, pane.pid);
    assert_eq!(pids(&rows), [52064, 52100]);
    assert_eq!(rows.iter().map(|(d, _)| *d).collect::<Vec<_>>(), [0, 1]);
}

#[test]
fn ordering_by_cpu_puts_the_busiest_pane_first() {
    let s = sample();
    let first = |order| ordered(&s.panes, order, None)[0].pane;
    assert_eq!(first(Order::Sidebar), 23);
    assert_eq!(first(Order::Cpu), 28);
    assert_eq!(first(Order::Memory), 28);
}

#[test]
fn a_held_order_keeps_the_panes_where_they_were_and_adds_new_ones_last() {
    let pane = |pane, cpu| PaneUse { pane, cpu: Some(cpu), ..PaneUse::default() };
    let panes = [pane(1, 90.), pane(2, 5.), pane(3, 50.)];
    let order = |held: Option<&[u64]>| ordered(&panes, Order::Cpu, held).iter().map(|p| p.pane).collect::<Vec<_>>();
    assert_eq!(order(None), [1, 3, 2]);
    assert_eq!(order(Some(&[2, 1])), [2, 1, 3]);
}

#[test]
fn a_closed_pane_hides_what_its_programs_started() {
    let s = sample();
    let pane = &s.panes[1];
    let fold = folded(under(&pane.processes, pane.pid), false, "");
    assert_eq!((pids(&fold.rows), fold.hidden), (vec![52064], 1));
    let open = folded(under(&pane.processes, pane.pid), true, "");
    assert_eq!((open.rows.len(), open.hidden), (2, 1));
}

fn chain() -> Vec<Process> {
    let processes = [process(1, None, 0.), process(2, Some(1), 0.), process(3, Some(2), 0.), process(4, Some(3), 99.9)];
    [processes.as_slice(), &[process(5, Some(2), 0.), process(6, Some(1), 0.)]].concat()
}

#[test]
fn a_closed_pane_still_shows_a_busy_process_and_the_ones_above_it() {
    let processes = chain();
    let fold = folded(under(&processes, 1), false, "");
    assert_eq!((pids(&fold.rows), fold.hidden), (vec![2, 3, 4, 6], 1));
}

#[test]
fn a_pane_found_through_a_process_shows_it_and_the_ones_above_it() {
    let processes = chain();
    let fold = folded(under(&processes, 1), false, " P3 ");
    assert_eq!((pids(&fold.rows), fold.hidden), (vec![2, 3], 3));
}

#[test]
fn a_filter_matches_the_place_the_program_or_a_command() {
    let s = sample();
    let python = &s.panes[1];
    assert_eq!(found(python, "shop › main › dev", ""), Found::Pane);
    assert_eq!(found(python, "shop › main › dev", "SHOP"), Found::Pane);
    assert_eq!(found(python, "shop › main › dev", "python"), Found::Pane);
    assert_eq!(found(python, "shop › main › dev", "http.server 0"), Found::Inside);
    assert_eq!(found(python, "shop › main › dev", "cargo"), Found::Not);
}

#[test]
fn the_place_names_the_pane_only_in_a_split_tab() {
    let pane = PaneUse { pane: 28, tab: 24, workspace: 22, ..PaneUse::default() };
    assert_eq!(place(&pane, Some(&session(1))), "shop › main › dev");
    assert_eq!(place(&pane, Some(&session(2))), "shop › main › dev · pane 28");
    assert_eq!(place(&pane, None), "pane 28");
}

#[test]
fn the_command_line_names_the_program_and_cuts_long_arguments() {
    let node =
        Process { name: "node".into(), args: vec!["/usr/bin/node".into(), "server.js".into()], ..Process::default() };
    assert_eq!(node.command(), ("node".to_string(), "server.js".to_string()));
    let long = command_line("x", &[String::new(), "a".repeat(100)]);
    assert_eq!(long.chars().count(), 60);
    assert!(long.ends_with('…'));
}

#[test]
fn arguments_holding_line_breaks_read_as_one_line() {
    let script = Process { args: vec!["sh".into(), "-c".into(), "cd x\nmake\tall".into()], ..Process::default() };
    assert_eq!(script.command().1, "-c cd x make all");
}

#[test]
fn a_process_is_named_by_the_program_it_runs() {
    let claude =
        Process { name: "2.1.311".into(), args: vec!["claude".into(), "--resume".into()], ..Process::default() };
    assert_eq!(claude.command().0, "claude");
}

#[test]
fn stopping_passes_the_start_when_it_is_known() {
    let known = Process { pid: 42, started: Some(7), ..Process::default() };
    assert_eq!(known.stop_command(), ["processes", "stop", "42", "--started", "7"]);
    let unknown = Process { pid: 42, ..Process::default() };
    assert_eq!(unknown.stop_command(), ["processes", "stop", "42"]);
}

#[test]
fn the_sample_command_narrows_to_a_workspace() {
    assert_eq!(sample_command(None), ["processes", "--json"]);
    assert_eq!(sample_command(Some(3)), ["processes", "--json", "--workspace", "3"]);
}

#[test]
fn figures_that_could_not_be_read_show_a_question_mark() {
    assert_eq!(cpu(None), "?");
    assert_eq!(memory(None), "?");
    assert_eq!(cpu(Some(12.345)), "12.3 %");
}

#[test]
fn cpu_reads_idle_below_what_shows_as_zero_and_hot_from_half_a_core() {
    assert_eq!(heat(None), Heat::Idle);
    assert_eq!(heat(Some(0.04)), Heat::Idle);
    assert_eq!(heat(Some(12.5)), Heat::Busy);
    assert_eq!(heat(Some(50.)), Heat::Hot);
    assert!((load(Some(250.)) - 1.).abs() < f32::EPSILON);
}

#[test]
fn the_footer_counts_processes_and_panes() {
    assert_eq!(counts(&sample()), "4 processes in 2 panes");
}

#[test]
fn a_pane_is_headed_by_its_agent_else_its_program() {
    let s = sample();
    assert_eq!(heading(&s.panes[0]), "zsh");
    let agent = PaneUse { agent: Some("claude".into()), program: Some("2.1.311".into()), ..PaneUse::default() };
    assert_eq!(heading(&agent), "claude");
}
