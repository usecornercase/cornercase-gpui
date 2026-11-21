use super::data::Body;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeysTo {
    Prompt,
    Filter,
    First,
    Wait,
}

pub(super) fn keys_go(on_top: bool, form: bool, body: Body) -> KeysTo {
    if !on_top {
        return KeysTo::Wait;
    }
    if form {
        return KeysTo::Prompt;
    }
    match body {
        Body::Loading => KeysTo::Wait,
        Body::Empty => KeysTo::First,
        Body::NoMatch | Body::List => KeysTo::Filter,
    }
}

pub(super) fn follows(before: KeysTo, now: KeysTo) -> bool {
    let page = |to: KeysTo| matches!(to, KeysTo::Filter | KeysTo::First);
    before != now && page(before) && page(now)
}

#[cfg(test)]
mod tests;
