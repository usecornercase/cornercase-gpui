use serde::Deserialize;

const MOST_TEXT: usize = 200;
const MOST_HTML: usize = 1500;
const MOST_ATTRIBUTES: usize = 12;
const MOST_BRIEF: usize = 4000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Rect {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Component {
    pub(crate) framework: String,
    pub(crate) name: String,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<u32>,
    pub(crate) approximate: bool,
    pub(crate) owners: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Element {
    pub(crate) url: String,
    pub(crate) tag: String,
    pub(crate) selector: String,
    pub(crate) id: String,
    pub(crate) classes: Vec<String>,
    pub(crate) text: String,
    pub(crate) attributes: Vec<(String, String)>,
    pub(crate) html: String,
    pub(crate) rect: Rect,
    pub(crate) page: Rect,
    pub(crate) styles: Vec<(String, String)>,
    pub(crate) component: Option<Component>,
}

pub(crate) fn bounded(text: &str, most: usize) -> String {
    cut(&text.split_whitespace().collect::<Vec<_>>().join(" "), most)
}

fn whole(n: f64) -> String {
    format!("{}", n.round())
}

impl Component {
    pub(crate) fn place(&self) -> Option<String> {
        let file = self.file.as_deref().filter(|f| !f.is_empty())?;
        let at = self.line.map_or_else(|| file.to_string(), |line| format!("{file}:{line}"));
        Some(if self.approximate { format!("near {at}") } else { at })
    }

    pub(crate) fn summary(&self) -> String {
        let name = if self.name.is_empty() { "a component".to_string() } else { self.name.clone() };
        match self.place() {
            Some(place) => format!("{name} · {place}"),
            None => name,
        }
    }

    fn line(&self) -> String {
        let mut line = format!("Component: {}", self.summary().replace(" · ", ", "));
        let framework = if self.framework.is_empty() { String::new() } else { format!(" ({})", self.framework) };
        line.push_str(&framework);
        if self.owners.len() > 1 {
            line = format!("{line}, inside {}", self.owners.join(" > "));
        }
        line
    }
}

impl Element {
    pub(crate) fn parse(value: serde_json::Value) -> Option<Self> {
        let element: Self = serde_json::from_value(value).ok()?;
        (!element.tag.is_empty()).then_some(element)
    }

    pub(crate) fn heading(&self) -> String {
        format!("<{}> {}", self.tag, self.selector)
    }

    pub(crate) fn detail(&self) -> Option<String> {
        if let Some(component) = &self.component {
            return Some(component.summary());
        }
        (!self.text.is_empty()).then(|| format!("“{}”", bounded(&self.text, 80)))
    }

    fn attributes_line(&self) -> Option<String> {
        let pairs: Vec<String> = self
            .attributes
            .iter()
            .take(MOST_ATTRIBUTES)
            .map(|(name, value)| format!("{name}=\"{}\"", bounded(value, MOST_TEXT)))
            .collect();
        (!pairs.is_empty()).then(|| format!("Attributes: {}", pairs.join(", ")))
    }

    fn styles_line(&self) -> String {
        let size = format!("size {}×{}", whole(self.rect.width), whole(self.rect.height));
        let styles = self.styles.iter().map(|(name, value)| format!("{name} {}", bounded(value, 120)));
        let all: Vec<String> = styles.chain(std::iter::once(size)).collect();
        format!("Styles: {}", all.join("; "))
    }

    pub(crate) fn brief(&self, cropped: bool) -> String {
        let opening = if cropped {
            "I picked this element in the browser (the image is a crop of it):"
        } else {
            "I picked this element in the browser:"
        };
        let lines = [
            Some(opening.to_string()),
            Some(format!("Page: {}", self.url)),
            Some(format!("Element: {}", self.heading())),
            self.component.as_ref().map(Component::line),
            (!self.text.is_empty()).then(|| format!("Text: \"{}\"", bounded(&self.text, MOST_TEXT))),
            self.attributes_line(),
            Some(self.styles_line()),
            (!self.html.is_empty()).then(|| format!("HTML: {}", bounded(&self.html, MOST_HTML))),
        ];
        let text = lines.into_iter().flatten().collect::<Vec<_>>().join("\n");
        cut(&text, MOST_BRIEF)
    }
}

pub(crate) fn cut(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_string();
    }
    let kept: String = text.chars().take(most.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests;
