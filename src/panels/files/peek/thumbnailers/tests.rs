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
fn the_type_comes_from_the_longest_glob_of_the_highest_weight() {
    assert_eq!(mime_of("Report.PDF", GLOBS).as_deref(), Some("application/pdf"));
    assert_eq!(mime_of("src.tar.gz", GLOBS).as_deref(), Some("application/x-compressed-tar"));
    assert_eq!(mime_of("notes.gz", GLOBS).as_deref(), Some("application/gzip"));
    assert_eq!(mime_of("Makefile", GLOBS).as_deref(), Some("text/x-makefile"));
    assert_eq!(mime_of("makefile", GLOBS), None);
    assert_eq!(mime_of("README", GLOBS), None);
}

#[test]
fn the_first_thumbnailer_for_the_type_that_can_run_is_picked() {
    let all = [parse(EVINCE).expect("an entry"), parse(FFMPEG).expect("an entry")];
    assert_eq!(pick(&all, "video/mp4", &|_| true), all.get(1));
    assert_eq!(pick(&all, "video/mp4", &|program| program != "ffmpegthumbnailer"), None);
    assert_eq!(pick(&all, "font/ttf", &|_| true), None);
}
