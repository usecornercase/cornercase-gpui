use super::*;
use crate::model::Colour;

#[test]
fn reads_a_summary_line() {
    let line = r#"{"boot":"1-2","error":null,"mode":"uncommitted","revision":3,"scope":"project","state":"ready","stream":1,
        "workspaces":[{"added":7,"base":null,"project":29,"removed":5,"workspace":28,"files":[
          {"added":0,"digest":"6113471e8352b284","fold":"open","old_path":null,"path":"gone.txt","removed":1,"staged":"no","status":"deleted","viewed":false},
          {"added":0,"digest":"c54c477b3900e220","fold":"open","old_path":"rename_me.txt","path":"renamed.txt","removed":0,"staged":"yes","status":"renamed","viewed":true},
          {"added":2,"digest":"a5c2fcc5bb9cebb1","fold":"binary","path":"x.png","removed":0,"status":"untracked","viewed":false}]}]}"#;
    let s = summary(line).expect("parses");
    assert_eq!((s.revision, s.state, s.mode), (3, State::Ready, Mode::Uncommitted));
    let w = &s.workspaces[0];
    assert_eq!((w.project, w.workspace, w.added, w.removed), (29, 28, 7, 5));
    assert_summary_files(&w.files);
}

fn assert_summary_files(files: &[FileInfo]) {
    assert_eq!(files[0].status, FileStatus::Deleted);
    assert_eq!(files[0].staged, Some(Staging::No));
    assert_eq!(files[1].old_path.as_deref(), Some("rename_me.txt"));
    assert!(files[1].viewed);
    assert_eq!(files[2].fold, FoldClass::Binary);
    assert_eq!(files[2].staged, None);
}

#[test]
fn unknown_values_do_not_break_a_summary() {
    let line = r#"{"state":"loading","mode":"commits","workspaces":[{"files":[{"path":"a","status":"copied","fold":"huge","new_field":1}]}]}"#;
    let s = summary(line).expect("parses");
    assert_eq!(s.state, State::Loading);
    assert_eq!(s.mode, Mode::Commits);
    assert_eq!(s.workspaces[0].files[0].status, FileStatus::Other);
    assert_eq!(s.workspaces[0].files[0].fold, FoldClass::Other);
}

#[test]
fn reads_a_failed_summary() {
    let s = summary(r#"{"state":"failed","error":"not a git repository","workspaces":[]}"#).expect("parses");
    assert_eq!(s.state, State::Failed);
    assert_eq!(s.error.as_deref(), Some("not a git repository"));
}

#[test]
fn reads_a_patch_with_styles_and_emphasis() {
    let json = r#"{"added":1,"digest":"550a7a517a389403","fold":"open","mode":"uncommitted","old_path":null,
        "path":"src/main.rs","removed":1,"staged":"partly","status":"modified","viewed":false,"workspace":28,
        "styles":[{},{"fg":{"index":5}},{"fg":{"rgb":[1,2,3]},"bold":true}],
        "hunks":[{"context":"fn main","new_start":1,"old_start":1,"lines":[
          {"kind":"context","new":1,"old":1,"text":"fn main() {","syntax":[{"style":1,"text":"fn"},{"style":0,"text":" main() {"}]},
          {"kind":"removed","new":null,"old":2,"text":"    a","emphasis":[[4,5]]},
          {"kind":"added","new":2,"old":null,"text":"    b","emphasis":[[4,5]]}]}]}"#;
    let p: Patch = serde_json::from_str(json).expect("parses");
    assert_eq!((p.workspace, p.file.path.as_str(), p.file.staged), (28, "src/main.rs", Some(Staging::Partly)));
    assert_eq!(p.styles[1].fg, Some(Colour::Index(5)));
    assert_eq!(p.styles[2].fg, Some(Colour::Rgb([1, 2, 3])));
    assert!(p.styles[2].bold);
    assert_patch_hunk(&p.hunks[0]);
}

fn assert_patch_hunk(h: &Hunk) {
    assert_eq!((h.old_start, h.new_start, h.context.as_str()), (1, 1, "fn main"));
    assert_eq!(h.lines[1].kind, LineKind::Removed);
    assert_eq!(h.lines[1].new, None);
    assert_eq!(h.lines[2].emphasis, vec![[4, 5]]);
    assert_eq!(h.lines[0].syntax.as_ref().map(Vec::len), Some(2));
}

#[test]
fn reads_a_gap_and_bases() {
    let gap: Gap = serde_json::from_str(
        r#"{"workspace":1,"path":"a.rs","hunk":2,"styles":[{}],"lines":[{"old":4,"new":5,"text":"x"}]}"#,
    )
    .expect("parses");
    assert_eq!((gap.hunk, gap.lines[0].old, gap.lines[0].new), (2, 4, 5));
    let base: Base =
        serde_json::from_str(r#"{"base":null,"branches":["feature","main"],"default":"main","workspace":28}"#)
            .expect("parses");
    assert_eq!(base.default.as_deref(), Some("main"));
    assert_eq!(base.branches, vec!["feature", "main"]);
}
