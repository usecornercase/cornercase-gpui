use std::path::PathBuf;

use crate::cli::{Remote, Target};
use crate::model::machines::MachineIcon;
use crate::model::{Aim, Session, Spot, ThereId};
use crate::store::{MachineRead, Reach};

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

pub(crate) fn vps_aim() -> Aim {
    Aim { machine: "vps".into(), boot: "28257-1793303771316290000".into(), workspace: ThereId::heard(7) }
}

pub(crate) fn vps_target() -> Target {
    Target::Remote(Remote::new("ana@vps.example.com", "").expect("a destination"))
}

pub(crate) fn vps_read(reach: Reach) -> MachineRead {
    MachineRead {
        aim: vps_aim(),
        icon: MachineIcon::Server,
        place: "shop".into(),
        path: "/home/ana/code/shop".into(),
        folder: "~/code/shop".into(),
        reach,
    }
}
