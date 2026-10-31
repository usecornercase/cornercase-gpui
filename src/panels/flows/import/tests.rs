use super::Request;

fn request() -> Request {
    Request { source: "q.toml".into(), name: " ".into(), ..Request::default() }
}

#[test]
fn imports_without_asking_once_checked() {
    assert_eq!(request().args(false), ["flow", "import", "q.toml", "--yes", "--json"]);
}

#[test]
fn a_check_never_passes_yes() {
    assert_eq!(request().args(true), ["flow", "import", "q.toml", "--check", "--json"]);
}

#[test]
fn a_project_import_names_the_project_and_the_options() {
    let request =
        Request { source: "-".into(), name: "Mine".into(), project: Some(3), to_project: true, replace: true };
    assert_eq!(
        request.args(true),
        [
            "flow",
            "import",
            "-",
            "--check",
            "--json",
            "--to",
            "project",
            "--project",
            "3",
            "--name",
            "Mine",
            "--replace"
        ]
    );
}
