use super::held::Hold;
use crate::model::MachineSeen;
use crate::model::machines::Machine;
use crate::model::tunnels::{Gate, Older, Toward, Way};
use crate::store::Reach;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Link {
    Up { boot: String, destination: String },
    Waiting,
    Lost,
}

pub(crate) fn machine_link(
    listed: Option<&Machine>,
    listing_known: bool,
    seen: Option<&MachineSeen>,
    reach: Option<&Reach>,
    gate: &Gate,
) -> Link {
    let Some(listed) = listed else { return if listing_known { Link::Lost } else { Link::Waiting } };
    let (Some(seen), Some(reach)) = (seen, reach) else { return Link::Lost };
    match (reach, gate) {
        (Reach::Refused(_), _) | (Reach::Up(_), Gate::Older(Older::ThisComputer(_) | Older::Server(Some(_)))) => {
            Link::Lost
        }
        (Reach::Down(_), _) | (Reach::Up(_), Gate::Waiting | Gate::Older(Older::Server(None))) => Link::Waiting,
        (Reach::Up(_), Gate::Open) => Link::Up { boot: seen.boot.clone(), destination: listed.destination.clone() },
    }
}

pub(crate) fn hold_stays(hold_destination: &str, link: &Link) -> bool {
    match link {
        Link::Lost => false,
        Link::Up { destination, .. } => destination == hold_destination,
        Link::Waiting => true,
    }
}

pub(crate) fn split_ended(holds: Vec<Hold>, way: &Way) -> (Vec<Hold>, Vec<Hold>) {
    holds.into_iter().partition(|hold| hold.toward == Toward::Server && *way != Way::Cornercase)
}

pub(crate) fn split_stale(holds: Vec<Hold>, toward: &Toward, link: &Link) -> (Vec<Hold>, Vec<Hold>) {
    holds.into_iter().partition(|hold| hold.toward == *toward && !hold_stays(&hold.destination, link))
}

#[cfg(test)]
mod tests;
