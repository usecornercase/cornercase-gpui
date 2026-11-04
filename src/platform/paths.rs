use std::path::{Component, Path, PathBuf};

use super::Os;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    Unix,
    Windows,
}

impl Default for Style {
    fn default() -> Self {
        Self::of(Os::CURRENT)
    }
}

impl Style {
    pub(crate) const fn of(os: Os) -> Self {
        match os {
            Os::Windows => Self::Windows,
            Os::Mac | Os::Linux => Self::Unix,
        }
    }

    pub(crate) fn of_path(path: &str) -> Self {
        if windows_rooted(path) { Self::Windows } else { Self::Unix }
    }

    pub(crate) fn is_separator(self, c: char) -> bool {
        c == '/' || (self == Self::Windows && c == '\\')
    }

    pub(crate) const fn separator(self) -> char {
        match self {
            Self::Unix => '/',
            Self::Windows => '\\',
        }
    }

    pub(crate) fn is_absolute(self, path: &str) -> bool {
        match self {
            Self::Unix => path.starts_with('/'),
            Self::Windows => windows_rooted(path),
        }
    }

    pub(crate) fn name_start(self, path: &str) -> usize {
        path.rfind(|c| self.is_separator(c)).map_or(0, |at| at + 1)
    }

    pub(crate) fn split(self, path: &str) -> (&str, &str) {
        let trimmed = path.trim_end_matches(|c| self.is_separator(c));
        let at = self.name_start(trimmed);
        let folder = &trimmed[..at];
        let bare = folder.trim_end_matches(|c| self.is_separator(c));
        let root = bare.is_empty() || (self == Self::Windows && bare.len() == 2 && bare.ends_with(':'));
        (if root { folder } else { bare }, &trimmed[at..])
    }

    pub(crate) fn name(self, path: &str) -> &str {
        self.split(path).1
    }

    pub(crate) fn in_form(self, path: &str) -> String {
        match self {
            Self::Unix => path.to_string(),
            Self::Windows => path.replace('/', "\\"),
        }
    }

    pub(crate) fn relative(self, path: &str) -> PathBuf {
        match self {
            Self::Unix => PathBuf::from(path),
            Self::Windows => PathBuf::from(path.replace('\\', "/")),
        }
    }

    pub(crate) fn shown(self, path: &Path) -> String {
        let mut out = String::new();
        for part in path.components() {
            match part {
                Component::Prefix(prefix) => out.push_str(&prefix.as_os_str().to_string_lossy()),
                Component::RootDir => out.push(self.separator()),
                other => self.push_part(&mut out, &other.as_os_str().to_string_lossy()),
            }
        }
        out
    }

    fn push_part(self, out: &mut String, part: &str) {
        if !out.is_empty() && !out.ends_with(|c| self.is_separator(c)) {
            out.push(self.separator());
        }
        out.push_str(part);
    }

    pub(crate) fn join(self, root: &Path, rel: &Path) -> PathBuf {
        let text = rel.to_string_lossy();
        if self.is_absolute(&text) || text.is_empty() {
            return if text.is_empty() { root.to_path_buf() } else { rel.to_path_buf() };
        }
        let mut out = root.to_string_lossy().into_owned();
        self.push_part(&mut out, &self.shown(rel));
        PathBuf::from(out)
    }
}

fn windows_rooted(path: &str) -> bool {
    let mut chars = path.chars();
    let drive = chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.next() == Some(':');
    (drive && matches!(chars.next(), Some('\\' | '/'))) || path.starts_with(r"\\")
}

pub(crate) fn rooted(path: &str) -> bool {
    Style::of_path(path).is_absolute(path)
}

pub(crate) fn on_a_drive(path: &str) -> bool {
    let plain = path.strip_prefix(r"\\?\").unwrap_or(path);
    windows_rooted(plain) && !plain.starts_with(r"\\")
}

fn alike(style: Style, a: char, b: char) -> bool {
    a == b
        || (style == Style::Windows
            && ((style.is_separator(a) && style.is_separator(b)) || a.to_uppercase().eq(b.to_uppercase())))
}

fn under<'a>(path: &'a str, folder: &str, style: Style) -> Option<&'a str> {
    let mut chars = path.char_indices();
    let same = folder.chars().all(|want| chars.next().is_some_and(|(_, got)| alike(style, got, want)));
    let rest = chars.next().map_or("", |(at, _)| &path[at..]);
    (same && (rest.is_empty() || rest.starts_with(|c| style.is_separator(c)))).then_some(rest)
}

pub(crate) fn home_relative(path: &str, home: &str) -> String {
    let style = Style::of_path(home);
    let home = home.trim_end_matches(|c| style.is_separator(c));
    match under(path, home, style).filter(|_| !home.is_empty()) {
        Some(rest) => format!("~{rest}"),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests;
