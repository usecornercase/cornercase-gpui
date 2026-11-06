use super::data::sentence;

pub(super) const UNITS: [(char, &str); 4] = [('m', "minutes"), ('h', "hours"), ('d', "days"), ('w', "weeks")];
pub(super) const DAYS: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Repeat {
    Every { amount: u32, unit: char, clock: Option<String> },
    Day(String),
    Weekday(String),
    Week(&'static str, String),
}

impl Repeat {
    pub(super) fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (head, clock) = text.split_once(' ').map_or((text, None), |(head, clock)| (head, Some(clock.trim())));
        match (head, clock) {
            ("day", Some(clock)) => Some(Self::Day(clock.to_string())),
            ("weekday", Some(clock)) => Some(Self::Weekday(clock.to_string())),
            (head, Some(clock)) if DAYS.contains(&head) => {
                DAYS.iter().find(|day| **day == head).map(|day| Self::Week(day, clock.to_string()))
            }
            (head, clock) => every(head, clock),
        }
    }

    pub(super) fn clock(&self) -> Option<&str> {
        match self {
            Self::Every { clock, .. } => clock.as_deref(),
            Self::Day(clock) | Self::Weekday(clock) | Self::Week(_, clock) => Some(clock),
        }
    }

    pub(super) fn words(&self) -> String {
        match self {
            Self::Every { amount, unit, clock } => {
                let at = clock.as_ref().map_or_else(String::new, |clock| format!(" at {clock}"));
                format!("every {}{at}", span(*amount, *unit))
            }
            Self::Day(clock) => format!("every day at {clock}"),
            Self::Weekday(clock) => format!("every weekday at {clock}"),
            Self::Week(day, clock) => format!("every {} at {clock}", sentence(day)),
        }
    }
}

fn every(head: &str, clock: Option<&str>) -> Option<Repeat> {
    let unit = head.chars().last().filter(|unit| UNITS.iter().any(|(letter, _)| letter == unit))?;
    let amount = head.strip_suffix(unit)?.parse().ok()?;
    Some(Repeat::Every { amount, unit, clock: clock.map(str::to_string) })
}

fn span(amount: u32, unit: char) -> String {
    let name = UNITS.iter().find(|(letter, _)| *letter == unit).map_or("", |(_, name)| name);
    if amount == 1 { name.trim_end_matches('s').to_string() } else { format!("{amount} {name}") }
}

pub(super) fn words(text: &str) -> String {
    Repeat::parse(text).map_or_else(|| format!("every {text}"), |repeat| repeat.words())
}

#[cfg(test)]
mod tests;
