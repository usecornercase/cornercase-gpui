use super::machines::seen::WorkspaceThere;
use super::tunnels::Toward;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Owner {
    Here(u64),
    There(WorkspaceThere),
    Nobody,
}

impl Owner {
    pub(crate) fn toward(&self, host: Option<&str>) -> Option<Toward> {
        match self {
            Self::There(ws) => Some(Toward::Machine(ws.machine().to_string())),
            Self::Here(_) | Self::Nobody => host.map(|_| Toward::Server),
        }
    }

    pub(crate) fn there(&self) -> Option<&WorkspaceThere> {
        match self {
            Self::There(ws) => Some(ws),
            Self::Here(_) | Self::Nobody => None,
        }
    }

    pub(crate) fn machine(&self) -> Option<&str> {
        self.there().map(WorkspaceThere::machine)
    }

    pub(crate) fn has_place(&self) -> bool {
        *self != Self::Nobody
    }
}

#[cfg(test)]
mod tests;
