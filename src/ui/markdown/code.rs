use crate::ui::highlight::{self, Span};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Code {
    lang: String,
    text: String,
    spans: Vec<Span>,
}

impl Code {
    pub(crate) fn new(lang: &str, text: &str) -> Self {
        let spans = code_text(text).map(|shown| highlight::fenced(lang, &shown)).unwrap_or_default();
        Self { lang: lang.to_string(), text: text.to_string(), spans }
    }

    pub(crate) fn plain(text: &str) -> Self {
        Self { lang: String::new(), text: text.to_string(), spans: Vec::new() }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    #[cfg(test)]
    pub(crate) fn lang(&self) -> &str {
        &self.lang
    }

    pub(crate) fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub(crate) fn retext(&mut self, text: &str) {
        if text != self.text {
            *self = Self::new(&self.lang, text);
        }
    }

    pub(super) fn shown(&self) -> Option<String> {
        code_text(&self.text)
    }
}

pub(super) fn language(info: &str) -> String {
    let word = info.trim().split([' ', ',', ':']).next().unwrap_or("");
    let word = word.trim_start_matches(['{', '.']).trim_end_matches('}');
    word.to_lowercase()
}

fn code_text(text: &str) -> Option<String> {
    let lines: Vec<String> = text.lines().map(|line| line.replace('\t', "    ")).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

#[cfg(test)]
mod tests;
