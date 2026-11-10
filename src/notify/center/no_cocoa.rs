use std::time::Duration;

use futures::channel::mpsc::UnboundedSender;

use super::Refusal;

pub(super) fn bundle_id() -> Option<String> {
    None
}

pub(super) fn install(_clicks: &UnboundedSender<String>) {}

pub(super) fn post(_banner: (&str, &str, &str), _answer: Duration) -> Result<(), Refusal> {
    Err(Refusal { allowed: true, error: Some("macOS notifications run only on macOS".into()) })
}
