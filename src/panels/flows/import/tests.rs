use super::import_args;

#[test]
fn builds_import_arguments() {
    assert_eq!(import_args("q.toml", " ", None, false, false), ["flow", "import", "q.toml", "--yes", "--json"]);
    assert_eq!(
        import_args("-", "Mine", Some(3), true, true),
        ["flow", "import", "-", "--yes", "--json", "--to", "project", "--project", "3", "--name", "Mine", "--replace"]
    );
}
