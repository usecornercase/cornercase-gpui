use super::data::Body;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeysTo {
    Prompt,
    Filter,
    First,
    Wait,
}

pub(super) fn keys_go(form: bool, body: Body) -> KeysTo {
    if form {
        return KeysTo::Prompt;
    }
    match body {
        Body::Loading => KeysTo::Wait,
        Body::Empty => KeysTo::First,
        Body::NoMatch | Body::List => KeysTo::Filter,
    }
}

#[cfg(test)]
mod tests;
