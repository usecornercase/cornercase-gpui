use std::path::Path;

use gpui::{Hsla, Styled as _, Svg, px, svg};

use crate::theme::{ANSI, colours, hsla};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tint {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Soft,
}

impl Tint {
    fn colour(self) -> Hsla {
        match self {
            Tint::Red => hsla(ANSI[1]),
            Tint::Green => hsla(ANSI[2]),
            Tint::Yellow => hsla(ANSI[3]),
            Tint::Blue => hsla(ANSI[4]),
            Tint::Magenta => hsla(ANSI[5]),
            Tint::Cyan => hsla(ANSI[6]),
            Tint::Soft => colours::soft().into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileIcon {
    path: &'static str,
    bytes: &'static [u8],
    tint: Tint,
}

macro_rules! seti {
    ($name:literal, $tint:ident) => {
        FileIcon {
            path: concat!("file-icons/", $name, ".svg"),
            bytes: include_bytes!(concat!("../../assets/file-icons/", $name, ".svg")),
            tint: Tint::$tint,
        }
    };
}

const FOLDER: FileIcon = seti!("folder", Blue);
const FOLDER_OPEN: FileIcon = seti!("folder-open", Blue);
const GENERIC: FileIcon = seti!("default", Soft);

const NAMES: &[(&[&str], FileIcon)] = &[
    (&["Cargo.toml"], seti!("rust", Red)),
    (&["Cargo.lock", "package-lock.json", "pnpm-lock.yaml", "go.sum"], seti!("lock", Soft)),
    (&["package.json"], seti!("npm", Red)),
    (&["tsconfig.json", "jsconfig.json"], seti!("tsconfig", Blue)),
    (
        &[
            "Dockerfile",
            "Containerfile",
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
            ".dockerignore",
        ],
        seti!("docker", Blue),
    ),
    (&["Makefile", "GNUmakefile"], seti!("makefile", Yellow)),
    (&[".gitignore", ".gitattributes", ".gitmodules", ".gitkeep"], seti!("git", Red)),
    (&[".env", ".envrc", ".env.local", ".env.example"], seti!("config", Yellow)),
    (&["README", "README.md"], seti!("info", Cyan)),
    (&["LICENSE", "LICENSE.md", "LICENSE.txt", "LICENCE", "COPYING"], seti!("license", Yellow)),
    (&[".editorconfig"], seti!("editorconfig", Soft)),
    (&["Gemfile", "Rakefile"], seti!("ruby", Red)),
    (&["go.mod"], seti!("go", Cyan)),
    (&[".bashrc", ".zshrc", ".profile", ".bash_profile"], seti!("shell", Green)),
];

const EXTENSIONS: &[(&[&str], FileIcon)] = &[
    (&["rs"], seti!("rust", Red)),
    (&["ts", "mts", "cts"], seti!("typescript", Blue)),
    (&["tsx"], seti!("react", Blue)),
    (&["js", "mjs", "cjs"], seti!("javascript", Yellow)),
    (&["jsx"], seti!("react", Cyan)),
    (&["py", "pyi"], seti!("python", Yellow)),
    (&["go"], seti!("go", Cyan)),
    (&["rb"], seti!("ruby", Red)),
    (&["java"], seti!("java", Red)),
    (&["kt", "kts"], seti!("kotlin", Magenta)),
    (&["swift"], seti!("swift", Red)),
    (&["c"], seti!("c", Blue)),
    (&["h"], seti!("c", Magenta)),
    (&["cpp", "cc", "cxx"], seti!("cpp", Blue)),
    (&["hpp", "hh", "hxx"], seti!("cpp", Magenta)),
    (&["cs"], seti!("c-sharp", Green)),
    (&["php"], seti!("php", Magenta)),
    (&["lua"], seti!("lua", Blue)),
    (&["sh", "bash", "zsh", "fish"], seti!("shell", Green)),
    (&["toml"], seti!("toml", Red)),
    (&["yaml", "yml"], seti!("yml", Magenta)),
    (&["json", "jsonc", "json5"], seti!("json", Yellow)),
    (&["md", "markdown", "mdx"], seti!("markdown", Blue)),
    (&["html", "htm"], seti!("html", Red)),
    (&["css"], seti!("css", Blue)),
    (&["scss", "sass"], seti!("sass", Magenta)),
    (&["sql", "db", "sqlite"], seti!("db", Cyan)),
    (&["vue"], seti!("vue", Green)),
    (&["svelte"], seti!("svelte", Red)),
    (&["graphql", "gql"], seti!("graphql", Magenta)),
    (&["ex", "exs"], seti!("elixir", Magenta)),
    (&["hs"], seti!("haskell", Magenta)),
    (&["scala"], seti!("scala", Red)),
    (&["dart"], seti!("dart", Cyan)),
    (&["zig"], seti!("zig", Yellow)),
    (&["tf", "tfvars"], seti!("terraform", Magenta)),
    (&["vim"], seti!("vim", Green)),
    (&["xml"], seti!("xml", Yellow)),
    (&["svg"], seti!("svg", Yellow)),
    (&["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico"], seti!("image", Magenta)),
    (&["pdf"], seti!("pdf", Red)),
    (&["zip", "tar", "gz", "tgz", "xz", "bz2", "7z", "zst", "rar"], seti!("zip", Yellow)),
    (&["lock", "lockb"], seti!("lock", Soft)),
    (&["csv", "tsv"], seti!("csv", Green)),
    (&["ini", "conf", "cfg"], seti!("config", Soft)),
    (&["env"], seti!("config", Yellow)),
    (&["ttf", "otf", "woff", "woff2"], seti!("font", Soft)),
    (&["mp3", "wav", "flac", "ogg"], seti!("audio", Cyan)),
    (&["mp4", "mov", "webm", "mkv"], seti!("video", Magenta)),
];

fn find(table: &[(&[&str], FileIcon)], key: &str) -> Option<FileIcon> {
    table.iter().find(|(keys, _)| keys.iter().any(|k| k.eq_ignore_ascii_case(key))).map(|(_, icon)| *icon)
}

fn kind(name: &str, dir: bool, open: bool) -> FileIcon {
    match (dir, open) {
        (true, true) => FOLDER_OPEN,
        (true, false) => FOLDER,
        (false, _) => find(NAMES, name)
            .or_else(|| Path::new(name).extension().and_then(|ext| ext.to_str()).and_then(|ext| find(EXTENSIONS, ext)))
            .unwrap_or(GENERIC),
    }
}

pub(crate) fn file_icon(name: &str, dir: bool, open: bool, size: f32) -> Svg {
    let icon = kind(name, dir, open);
    svg().path(icon.path).size(px(size)).flex_none().text_color(icon.tint.colour())
}

fn every() -> impl Iterator<Item = FileIcon> {
    [FOLDER, FOLDER_OPEN, GENERIC].into_iter().chain(NAMES.iter().chain(EXTENSIONS).map(|(_, icon)| *icon))
}

pub(crate) fn load(path: &str) -> Option<&'static [u8]> {
    every().find(|icon| icon.path == path).map(|icon| icon.bytes)
}

pub(crate) fn paths() -> Vec<&'static str> {
    let mut paths: Vec<&'static str> = every().map(|icon| icon.path).collect();
    paths.sort_unstable();
    paths.dedup();
    paths
}

#[cfg(test)]
mod tests;
