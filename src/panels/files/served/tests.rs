use super::*;

fn kind(content: &Content) -> String {
    match content {
        Content::Image(image) => format!("{:?} image", image.format),
        Content::Gone(e) => format!("Gone: {e}"),
        other => format!("{other:?}").split('(').next().unwrap_or_default().to_string(),
    }
}

fn names(listing: &tree::Listing) -> Vec<(&str, bool)> {
    listing.entries.iter().map(|e| (e.name.as_str(), e.dir)).collect()
}

#[test]
fn a_listing_answers_each_folder_in_the_order_asked() {
    let folders = vec![PathBuf::new(), PathBuf::from("src"), PathBuf::from("nope")];
    let listed = listings(folders.clone(), Ok(crate::test_util::fixture!("files/list.json").to_string()));
    assert_eq!(listed.iter().map(|(rel, _)| rel.clone()).collect::<Vec<_>>(), folders);
    let root = listed[0].1.as_ref().expect("root listed");
    assert_eq!(names(root)[..3], [(".hidden_dir", true), ("docs", true), ("src", true)]);
    assert!(!root.truncated);
    assert_eq!(names(listed[1].1.as_ref().expect("src listed")), [("main.rs", false), ("new.rs", false)]);
}

#[test]
fn a_folder_that_cannot_be_listed_carries_its_error() {
    let folders = vec![PathBuf::new(), PathBuf::from("src"), PathBuf::from("nope")];
    let listed = listings(folders, Ok(crate::test_util::fixture!("files/list.json").to_string()));
    let error = listed[2].1.as_ref().expect_err("nope fails");
    assert_eq!(error.to_string(), "no such folder");
}

#[test]
fn a_failed_command_fails_every_folder_with_its_reason() {
    let listed = listings(vec![PathBuf::new(), PathBuf::from("src")], Err(anyhow::anyhow!("ssh: connection refused")));
    assert!(listed.iter().all(|(_, l)| l.as_ref().is_err_and(|e| e.to_string() == "ssh: connection refused")));
}

#[test]
fn a_text_file_reads_as_lines_with_its_stamp() {
    let (content, stamp) =
        content(Path::new("src/main.rs"), Ok(crate::test_util::fixture!("files/read-text.json").to_string()));
    let Content::Text(text) = content else { panic!("not text: {content:?}") };
    assert_eq!(text.lines, ["fn main() {", "    println!(\"hello\");", "}", "mod"]);
    let stamp = stamp.expect("a stamp");
    assert_eq!(stamp.len, 38);
    assert_eq!(stamp.modified, parse_utc("2026-10-29T20:21:00.503Z"));
}

#[test]
fn an_image_is_decoded_from_its_bytes() {
    let (content, _) =
        content(Path::new("tiny.png"), Ok(crate::test_util::fixture!("files/read-image.json").to_string()));
    assert_eq!(kind(&content), "Png image");
}

#[test]
fn an_svg_answered_as_text_shows_as_an_image() {
    let json = r#"{"kind":"text","bytes":41,"modified":null,"image":null,"text":"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"}"#;
    assert_eq!(kind(&content(Path::new("a.svg"), Ok(json.to_string())).0), "Svg image");
}

#[test]
fn other_kinds_say_what_the_file_is() {
    let read = |json: &str| kind(&content(Path::new("f"), Ok(json.to_string())).0);
    assert_eq!(read(crate::test_util::fixture!("files/read-binary.json")), "Binary");
    assert_eq!(read(crate::test_util::fixture!("files/read-too-large.json")), "TooLarge");
    assert_eq!(read(crate::test_util::fixture!("files/read-folder.json")), "NotRegular");
    assert_eq!(read(crate::test_util::fixture!("files/read-missing.json")), "Gone: No such file");
}

#[test]
fn a_missing_file_has_no_stamp() {
    let (_, stamp) =
        content(Path::new("nope.rs"), Ok(crate::test_util::fixture!("files/read-missing.json").to_string()));
    assert_eq!(stamp, None);
}

#[test]
fn a_read_that_fails_says_why() {
    let (content, _) = content(Path::new("f"), Err(anyhow::anyhow!("no workspace 9")));
    assert_eq!(kind(&content), "Gone: no workspace 9");
}

#[test]
fn a_file_stamp_comes_from_its_folder_listing() {
    let stamp = stamp_in(crate::test_util::fixture!("files/list.json"), "tiny.png").expect("listed");
    assert_eq!(stamp.len, 69);
    assert_eq!(stamp.modified, parse_utc("2026-10-29T20:21:03.961Z"));
    assert_eq!(stamp_in(crate::test_util::fixture!("files/list.json"), "gone.rs"), None);
}

#[test]
fn utc_times_parse_to_the_epoch() {
    assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(UNIX_EPOCH));
    assert_eq!(parse_utc("2000-03-01T00:00:01.5Z"), Some(UNIX_EPOCH + Duration::new(951_868_801, 500_000_000)));
    assert_eq!(parse_utc("2026-10-29T20:21:00.478Z"), Some(UNIX_EPOCH + Duration::new(1_793_305_260, 478_000_000)));
    assert_eq!(parse_utc("yesterday"), None);
}

#[test]
fn text_matches_light_their_characters() {
    let Results::Text(text) = results(Mode::Text, Ok(crate::test_util::fixture!("files/search-text.json").to_string()))
    else {
        panic!("not text results")
    };
    assert_eq!(text.count(), 4);
    let uni = text.files.iter().find(|f| f.path == "docs/uni.md").expect("uni.md");
    let hit = &uni.lines[0];
    assert_eq!(&hit.text[hit.lit[0].clone()], "hello");
    let main = text.files.iter().find(|f| f.path == "src/main.rs").expect("main.rs");
    assert_eq!(main.lines[0].line, 2);
}

#[test]
fn name_matches_light_their_characters() {
    let Results::Names(hits) =
        results(Mode::Names, Ok(crate::test_util::fixture!("files/search-names.json").to_string()))
    else {
        panic!("not name results")
    };
    assert_eq!(hits.len(), 1);
    assert_eq!(&hits[0].path[hits[0].lit[0].clone()], "uni");
}

#[test]
fn a_search_that_fails_says_why() {
    let failed = results(Mode::Names, Err(anyhow::anyhow!("ssh: timeout")));
    assert!(matches!(failed, Results::Failed(e) if e == "ssh: timeout"));
}

#[test]
fn paths_are_passed_after_the_options_and_never_as_home_or_flags() {
    let args = files_args("list", 3, &[], [arg(Path::new("")), arg(Path::new("~x/-y"))]);
    assert_eq!(args, ["files", "list", "--workspace", "3", "--json", "--", ".", "./~x/-y"]);
}

#[test]
fn a_path_outside_the_workspace_goes_as_it_is() {
    assert_eq!(arg(Path::new("/Users/me/plan.md")), "/Users/me/plan.md");
}
