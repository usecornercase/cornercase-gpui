#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Before(u64),
    End,
}

pub(crate) fn drop_place(siblings: &[u64], dragged: u64, target: u64) -> Option<Place> {
    if dragged == target {
        return None;
    }
    let at = siblings.iter().position(|&id| id == target)?;
    match siblings.iter().position(|&id| id == dragged) {
        Some(from) if from < at => {
            Some(siblings[at + 1..].iter().find(|&&id| id != dragged).map_or(Place::End, |&id| Place::Before(id)))
        }
        _ => Some(Place::Before(target)),
    }
}

pub(crate) fn lands_after(siblings: &[u64], dragged: u64, target: u64) -> bool {
    let pos = |id| siblings.iter().position(|&x| x == id);
    matches!((pos(dragged), pos(target)), (Some(a), Some(b)) if a < b)
}

#[cfg(test)]
mod tests;
