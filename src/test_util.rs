use std::path::PathBuf;

macro_rules! fixture {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/", $path))
    };
}

pub(crate) use fixture;

pub(crate) fn in_usr_bin(tools: &'static [&'static str]) -> impl Fn(&str) -> Option<PathBuf> {
    move |name| tools.contains(&name).then(|| PathBuf::from(format!("/usr/bin/{name}")))
}
