use super::{Making, Offer};

pub(crate) const IDEAS: [(&str, &str); 3] = [
    ("PRs", "Every weekday at 9:00, review my open pull requests and tell me which are blocked"),
    ("News", "Every morning at 8:00, sum up the news about the tools I use"),
    ("Releases", "Every Monday, check the new releases of the tools I use and tell me what changed"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Lobby {
    Empty { sets_itself_up: bool },
    Pick,
    Lacks(String),
}

pub(crate) fn lobby_of(offer: &Offer, making: Option<&Making>, sets_itself_up: bool) -> Lobby {
    match (offer, making) {
        (Offer::Lacks(words), _) | (Offer::Hidden, Some(Making::Here(words) | Making::Server(words))) => {
            Lobby::Lacks(words.clone())
        }
        (Offer::Rows, _) => Lobby::Pick,
        (Offer::Hidden, _) => Lobby::Empty { sets_itself_up },
    }
}

impl Lobby {
    pub(crate) fn heading(&self) -> &'static str {
        match self {
            Self::Empty { .. } => "Meet your bots",
            Self::Pick => "Your bots",
            Self::Lacks(_) => "Bots",
        }
    }

    pub(crate) fn lines(&self) -> Vec<String> {
        const WHAT: &str = "A bot is an agent of your own, outside any project: a name, a folder, a memory and a chat.";
        match self {
            Self::Empty { sets_itself_up: true } => vec![
                WHAT.to_string(),
                "Tell it what it is for and it sets itself up, schedules its own work and answers here.".to_string(),
            ],
            Self::Empty { sets_itself_up: false } => {
                vec![WHAT.to_string(), "Write to it and it answers here.".to_string()]
            }
            Self::Pick => vec!["Pick a bot on the left to read its chat and talk to it.".to_string()],
            Self::Lacks(words) => vec![WHAT.to_string(), words.clone()],
        }
    }

    pub(crate) fn makes(&self) -> bool {
        !matches!(self, Self::Lacks(_))
    }

    pub(crate) fn ideas(&self) -> bool {
        *self == Self::Empty { sets_itself_up: true }
    }
}

#[cfg(test)]
mod tests;
