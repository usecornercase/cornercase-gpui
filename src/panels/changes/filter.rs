use super::wire::FileInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pattern {
    Part(String),
    Glob(String),
}

impl Pattern {
    fn new(word: &str) -> Self {
        if word.contains(['*', '?']) { Self::Glob(word.to_string()) } else { Self::Part(word.to_string()) }
    }

    fn matches(&self, path: &str) -> bool {
        match self {
            Self::Part(part) => {
                let fold = ignores_case(part);
                let part = letters(part, fold);
                letters(path, fold).windows(part.len()).any(|w| w == part.as_slice())
            }
            Self::Glob(glob) => glob_matches(glob, path),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Query {
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
}

impl Query {
    pub(super) fn parse(raw: &str) -> Self {
        let mut query = Self::default();
        for word in raw.split_whitespace() {
            match word.strip_prefix('!') {
                Some(left_out) if !left_out.is_empty() => query.exclude.push(Pattern::new(left_out)),
                _ => query.include.push(Pattern::new(word)),
            }
        }
        query
    }

    pub(super) fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty()
    }

    pub(super) fn keeps(&self, file: &FileInfo) -> bool {
        let paths = [Some(file.path.as_str()), file.old_path.as_deref()];
        let matches = |pattern: &Pattern| paths.iter().flatten().any(|path| pattern.matches(path));
        (self.include.is_empty() || self.include.iter().any(matches)) && !self.exclude.iter().any(matches)
    }
}

pub(super) fn files_label(kept: usize, total: usize, filtering: bool) -> String {
    match (filtering, total) {
        (true, _) => format!("{kept} of {total} files"),
        (false, 1) => "1 file".to_string(),
        (false, _) => format!("{total} files"),
    }
}

fn letters(text: &str, fold: bool) -> Vec<char> {
    text.chars().map(|c| if fold { c.to_lowercase().next().unwrap_or(c) } else { c }).collect()
}

fn ignores_case(word: &str) -> bool {
    !word.chars().any(char::is_uppercase)
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    let fold = ignores_case(pattern);
    let (pattern, only_folders) = pattern.strip_suffix('/').map_or((pattern, false), |p| (p, true));
    let (pattern, anchored) = pattern.strip_prefix('/').map_or((pattern, false), |p| (p, true));
    if anchored || pattern.contains('/') {
        let below = letters(&format!("{pattern}/**"), fold);
        let path = letters(path, fold);
        return (!only_folders && glob(&letters(pattern, fold), &path)) || glob(&below, &path);
    }
    let pattern = letters(pattern, fold);
    let parts: Vec<&str> = path.split('/').collect();
    let last = parts.len().saturating_sub(1);
    parts.iter().enumerate().any(|(i, part)| (!only_folders || i < last) && glob(&pattern, &letters(part, fold)))
}

fn glob(pattern: &[char], text: &[char]) -> bool {
    match pattern {
        [] => text.is_empty(),
        ['*', '*', rest @ ..] => any_depth(rest, text),
        ['*', rest @ ..] => {
            let end = text.iter().position(|c| *c == '/').unwrap_or(text.len());
            (0..=end).any(|i| glob(rest, &text[i..]))
        }
        ['?', rest @ ..] => text.first().is_some_and(|c| *c != '/') && glob(rest, &text[1..]),
        [c, rest @ ..] => text.first() == Some(c) && glob(rest, &text[1..]),
    }
}

fn any_depth(rest: &[char], text: &[char]) -> bool {
    let at_a_folder =
        |after: &[char]| (0..=text.len()).filter(|&i| i == 0 || text[i - 1] == '/').any(|i| glob(after, &text[i..]));
    rest.strip_prefix(&['/']).is_some_and(at_a_folder) || (0..=text.len()).any(|i| glob(rest, &text[i..]))
}

#[cfg(test)]
mod tests;
