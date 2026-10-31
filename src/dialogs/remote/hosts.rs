use std::path::{Path, PathBuf};

const MAX_DEPTH: u8 = 8;
const FORGES: [&str; 7] = [
    "github.com",
    "ssh.github.com",
    "gitlab.com",
    "altssh.gitlab.com",
    "bitbucket.org",
    "codeberg.org",
    "ssh.dev.azure.com",
];

#[derive(Debug, Default, PartialEq, Eq)]
struct Parsed {
    hosts: Vec<String>,
    includes: Vec<String>,
}

pub(super) fn read() -> Vec<String> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return Vec::new() };
    let mut hosts = Vec::new();
    read_file(&home.join(".ssh").join("config"), &home, 0, &mut hosts);
    hosts
}

fn read_file(path: &Path, home: &Path, depth: u8, hosts: &mut Vec<String>) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let parsed = parse(&text);
    for host in parsed.hosts {
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    if depth == MAX_DEPTH {
        return;
    }
    for file in parsed.includes.iter().flat_map(|pattern| included(pattern, home)) {
        read_file(&file, home, depth + 1, hosts);
    }
}

fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    for (keyword, rest) in text.lines().filter_map(keyword_and_rest) {
        let words = rest.split_whitespace().map(|word| word.trim_matches('"').to_string());
        if keyword.eq_ignore_ascii_case("host") {
            parsed.hosts.extend(words.filter(|word| is_server(word)));
        } else if keyword.eq_ignore_ascii_case("include") {
            parsed.includes.extend(words);
        }
    }
    parsed
}

fn is_server(name: &str) -> bool {
    !name.is_empty() && !name.contains(['*', '?', '!']) && !FORGES.contains(&name.to_ascii_lowercase().as_str())
}

fn keyword_and_rest(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.starts_with('#') {
        return None;
    }
    let (keyword, rest) = line.split_at(line.find(|c: char| c.is_whitespace() || c == '=')?);
    Some((keyword, rest.trim_start().trim_start_matches('=').trim_start()))
}

fn included(pattern: &str, home: &Path) -> Vec<PathBuf> {
    let path = match pattern.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => home.join(".ssh").join(pattern),
    };
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if !name.contains(['*', '?']) {
        return vec![path];
    }
    let Some(entries) = path.parent().and_then(|folder| std::fs::read_dir(folder).ok()) else { return Vec::new() };
    let pattern: Vec<char> = name.chars().collect();
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|file| {
            file.file_name().is_some_and(|n| glob(&pattern, &n.to_string_lossy().chars().collect::<Vec<_>>()))
        })
        .collect();
    files.sort();
    files
}

fn glob(pattern: &[char], name: &[char]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some(('*', rest)) => (0..=name.len()).any(|skip| glob(rest, &name[skip..])),
        Some(('?', rest)) => !name.is_empty() && glob(rest, &name[1..]),
        Some((c, rest)) => name.first() == Some(c) && glob(rest, &name[1..]),
    }
}

#[cfg(test)]
mod tests;
