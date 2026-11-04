use super::*;

const FFMPEG: &str = "[Thumbnailer Entry]\nTryExec=ffmpegthumbnailer\nExec=ffmpegthumbnailer -i %i -o %o -s %s -f\n\
                      MimeType=video/mp4;video/webm;\n";

const EVINCE: &str = "# comment\n[Thumbnailer Entry]\nExec=/usr/bin/evince-thumbnailer -s %s %u %o\n\
                      MimeType=application/pdf;\n\n[Other]\nExec=nothing\n";

const GLOBS: &str = "# glob file\n50:application/pdf:*.pdf\n50:application/gzip:*.gz\n\
                     55:application/x-compressed-tar:*.tar.gz\n50:text/x-makefile:Makefile:cs\n\
                     50:video/mp4:*.mp4\n";

#[test]
fn a_thumbnailer_file_names_its_program_command_and_types() {
    let ffmpeg = parse(FFMPEG).expect("an entry");
    assert_eq!(ffmpeg.program(), "ffmpegthumbnailer");
    assert!(ffmpeg.reads("video/webm"));
    assert!(!ffmpeg.reads("video/x-matroska"));
    let evince = parse(EVINCE).expect("an entry");
    assert_eq!(evince.program(), "/usr/bin/evince-thumbnailer");
    assert_eq!(evince.exec, "/usr/bin/evince-thumbnailer -s %s %u %o");
    assert_eq!(parse("[Thumbnailer Entry]\nMimeType=a/b;\n"), None);
}

#[test]
fn its_command_line_gets_the_file_the_picture_and_the_size() {
    let exec = "ffmpegthumbnailer -i %i -o \"%o\" -s %s -f 100%%";
    let video = argv(exec, Path::new("/w/my clip.mp4"), Path::new("/t/out.png"), 900).expect("an argv");
    assert_eq!(video, ["ffmpegthumbnailer", "-i", "/w/my clip.mp4", "-o", "/t/out.png", "-s", "900", "-f", "100%"]);
    let pdf = argv("thumb %u %o", Path::new("/w/a b.pdf"), Path::new("/t/o.png"), 64).expect("an argv");
    assert_eq!(pdf, ["thumb", "file:///w/a%20b.pdf", "/t/o.png"]);
    assert_eq!(argv("thumb %i", Path::new("relative"), Path::new("/t/o.png"), 64), None);
}

#[test]
fn its_command_line_is_split_as_the_desktop_entry_spec_quotes_it() {
    let exec = r#"thumb --size="%s" '%i' "a \"b\" \$c \d" plain\ word "" end"#;
    assert_eq!(words(exec), ["thumb", "--size=%s", "%i", r#"a "b" $c \d"#, "plain word", "", "end"]);
    let quoted = parse("[Thumbnailer Entry]\nExec=\"/opt/my thumb/run\" %i %o\n").expect("an entry");
    assert_eq!(quoted.program(), "/opt/my thumb/run");
    let video = argv(exec, Path::new("/w/a.mp4"), Path::new("/t/o.png"), 900).expect("an argv");
    assert_eq!(video[1], "--size=900");
    assert_eq!(video[2], "/w/a.mp4");
}

#[cfg(unix)]
#[test]
fn a_path_that_is_not_utf8_reaches_the_thumbnailer_as_it_is() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    let path = Path::new(OsStr::from_bytes(b"/w/caf\xe9.mp4"));
    let argv = argv("thumb %i %o", path, Path::new("/t/o.png"), 64).expect("an argv");
    assert_eq!(argv[1].as_bytes(), b"/w/caf\xe9.mp4");
}

#[test]
fn the_type_comes_from_the_longest_glob_of_the_highest_weight() {
    assert_eq!(mime_of("Report.PDF", GLOBS).as_deref(), Some("application/pdf"));
    assert_eq!(mime_of("src.tar.gz", GLOBS).as_deref(), Some("application/x-compressed-tar"));
    assert_eq!(mime_of("notes.gz", GLOBS).as_deref(), Some("application/gzip"));
    assert_eq!(mime_of("Makefile", GLOBS).as_deref(), Some("text/x-makefile"));
    assert_eq!(mime_of("makefile", GLOBS), None);
    assert_eq!(mime_of("README", GLOBS), None);
}

#[test]
fn globs_match_as_fnmatch_does() {
    let cases = [
        ("*.anim[1-9j]", "clip.anim5", true),
        ("*.anim[1-9j]", "clip.animj", true),
        ("*.anim[1-9j]", "clip.anim0", false),
        ("*.[!a-z]", "page.1", true),
        ("*.[!a-z]", "page.x", false),
        ("[]x]", "]", true),
        ("README*", "README.md", true),
        ("*.?z", "a.gz", true),
        ("*.?z", "a.z", false),
        ("*a*b*c", "xaybzbc", true),
        ("a[b", "a[b", true),
        ("\\*", "*", true),
        ("\\*", "x", false),
        ("*", "", true),
        ("Makefile", "Makefile.am", false),
    ];
    for (glob, name, expected) in cases {
        assert_eq!(glob_matches(glob, &name.chars().collect::<Vec<char>>()), expected, "{glob} against {name}");
    }
}

#[test]
fn a_type_is_found_by_any_glob_shared_mime_info_writes() {
    let globs = "50:video/x-anim:*.anim[1-9j]\n50:text/x-readme:README*\n";
    assert_eq!(mime_of("clip.ANIM5", globs).as_deref(), Some("video/x-anim"));
    assert_eq!(mime_of("README.first", globs).as_deref(), Some("text/x-readme"));
}

#[test]
fn the_globs_of_every_data_folder_are_ranked_together() {
    let user = "50:application/gzip:*.gz\n50:text/x-mine:*.foo\n".to_string();
    let system = "55:application/x-compressed-tar:*.tar.gz\n50:text/x-theirs:*.foo\n50:video/mp4:*.mp4\n".to_string();
    let files = [user, system];
    assert_eq!(mime_in("src.tar.gz", &files).as_deref(), Some("application/x-compressed-tar"));
    assert_eq!(mime_in("notes.gz", &files).as_deref(), Some("application/gzip"));
    assert_eq!(mime_in("a.foo", &files).as_deref(), Some("text/x-mine"));
    assert_eq!(mime_in("a.mp4", &files).as_deref(), Some("video/mp4"));
    assert_eq!(mime_in("a.mp4", &[]), None);
}

#[test]
fn the_first_thumbnailer_for_the_type_that_can_run_is_picked() {
    let all = [parse(EVINCE).expect("an entry"), parse(FFMPEG).expect("an entry")];
    assert_eq!(pick(&all, "video/mp4", &|_| true), all.get(1));
    assert_eq!(pick(&all, "video/mp4", &|program| program != "ffmpegthumbnailer"), None);
    assert_eq!(pick(&all, "font/ttf", &|_| true), None);
}

fn thumbnailer(exec: &str) -> Thumbnailer {
    Thumbnailer { try_exec: None, exec: exec.to_string(), types: vec!["video/mp4".into()] }
}

#[test]
fn a_thumbnailer_that_fails_is_named_with_what_went_wrong() {
    let file = Path::new("/w/a.mp4");
    let made = made_by(&thumbnailer("sh -c 'printf picture > \"$0\"' %o"), file);
    assert_eq!(made, Ok(b"picture".to_vec()));
    let failed = made_by(&thumbnailer("/bin/sh -c 'exit 2'"), file);
    assert_eq!(failed, Err(Missing::Failed("sh ended with exit status: 2".into())));
    let silent = made_by(&thumbnailer("true %o"), file);
    assert_eq!(silent, Err(Missing::Failed("true made no picture".into())));
}
