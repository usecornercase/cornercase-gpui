use serde::Deserialize;
use serde_json::Value;

const PREVIEW_MAX: usize = 120;
const LEADS: [&str; 6] = ["> ", "- ", "* ", "+ ", "[ ] ", "[x] "];
pub(crate) const FACE_COLOURS: [u8; 12] = [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14];

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Last {
    pub kind: String,
    pub text: String,
    pub at: i64,
}

pub(crate) fn last_or_none<'de, D: serde::Deserializer<'de>>(found: D) -> Result<Option<Last>, D::Error> {
    let value = Value::deserialize(found)?;
    Ok(Last::deserialize(value).ok().filter(|last| !last.kind.is_empty() || !last.text.is_empty()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Says {
    Asks,
    Thinking,
    Line(String),
    Nothing,
}

pub(crate) fn says(last: Option<&Last>, (asks, busy): (bool, bool)) -> Says {
    if asks {
        return Says::Asks;
    }
    if busy {
        return Says::Thinking;
    }
    last.and_then(line_of).map_or(Says::Nothing, Says::Line)
}

fn line_of(last: &Last) -> Option<String> {
    let text = plain(&last.text)?;
    let shown = match last.kind.as_str() {
        "you" => format!("You: {text}"),
        _ => text,
    };
    Some(shown)
}

fn plain(text: &str) -> Option<String> {
    let line = text.lines().map(unmarked).find(|line| !line.is_empty())?;
    Some(cut(&line))
}

fn without_lead(line: &str) -> &str {
    let mut line = line.trim();
    loop {
        let heading = line.trim_start_matches('#');
        let unled = [heading.strip_prefix(' ').filter(|_| heading.len() < line.len())]
            .into_iter()
            .chain(LEADS.iter().map(|lead| line.strip_prefix(lead)))
            .flatten()
            .next();
        match unled {
            Some(rest) => line = rest.trim_start(),
            None => return line,
        }
    }
}

fn unmarked(line: &str) -> String {
    let line = without_lead(line);
    let line = line.replace("**", "").replace("__", "").replace("~~", "").replace('`', "");
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cut(line: &str) -> String {
    match line.char_indices().nth(PREVIEW_MAX) {
        Some((at, _)) => format!("{}…", line[..at].trim_end()),
        None => line.to_string(),
    }
}

pub(crate) fn face_colour(id: &str) -> u8 {
    let sum = id.bytes().fold(5381_u32, |hash, byte| hash.wrapping_mul(33) ^ u32::from(byte));
    let at = usize::try_from(sum).unwrap_or_default() % FACE_COLOURS.len();
    FACE_COLOURS.get(at).copied().unwrap_or(4)
}

#[cfg(test)]
mod tests;
