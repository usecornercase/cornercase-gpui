use std::path::PathBuf;

use crate::model::{Session, Spot};

macro_rules! fixture {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/", $path))
    };
}

pub(crate) use fixture;

pub(crate) fn shop_on_vps() -> Spot {
    let session: Session = serde_json::from_str(fixture!("machines/view.json")).expect("a view line");
    let seen = session.machine("vps").expect("vps");
    seen.spot(seen.projects.iter().find(|there| there.name == "shop").expect("shop there"))
}

pub(crate) fn in_usr_bin(tools: &'static [&'static str]) -> impl Fn(&str) -> Option<PathBuf> {
    move |name| tools.contains(&name).then(|| PathBuf::from(format!("/usr/bin/{name}")))
}
