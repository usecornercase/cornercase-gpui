#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Enter {
    #[default]
    Breaks,
    Sends,
}

impl Enter {
    pub(super) const fn context(self) -> &'static str {
        match self {
            Self::Breaks => "Multiline",
            Self::Sends => "Chat",
        }
    }

    pub(super) const fn key_context(self) -> &'static str {
        match self {
            Self::Breaks => "TextField Multiline",
            Self::Sends => "TextField Chat",
        }
    }
}

pub(super) fn submits(enter: Enter, composing: bool) -> bool {
    enter != Enter::Sends || !composing
}

pub(super) fn passes_arrows(enter: Enter, empty: bool, several_rows: bool) -> bool {
    enter == Enter::Sends && (empty || !several_rows)
}
