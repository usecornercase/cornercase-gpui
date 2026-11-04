use std::path::Path;

use embed_resource::ParamsMacrosAndIncludeDirs;

const FOLDER: &str = "assets/windows";
const RESOURCES: &str = "assets/windows/cornercase-gpui.rc";
const INPUTS: [&str; 3] = [RESOURCES, "assets/windows/cornercase-gpui.manifest", "assets/windows/cornercase-gpui.ico"];

fn version_macros() -> [String; 4] {
    [
        format!("VERSION_MAJOR={}", env!("CARGO_PKG_VERSION_MAJOR")),
        format!("VERSION_MINOR={}", env!("CARGO_PKG_VERSION_MINOR")),
        format!("VERSION_PATCH={}", env!("CARGO_PKG_VERSION_PATCH")),
        format!("VERSION_TEXT=\"{}\"", env!("CARGO_PKG_VERSION")),
    ]
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    for input in INPUTS {
        println!("cargo::rerun-if-changed={input}");
    }
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join(FOLDER);
    let parameters = ParamsMacrosAndIncludeDirs(version_macros(), [folder]);
    if let Err(why) = embed_resource::compile(RESOURCES, parameters).manifest_required() {
        panic!("could not embed {RESOURCES}: {why}");
    }
}
