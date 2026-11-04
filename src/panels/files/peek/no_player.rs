use std::path::Path;

use gpui::AnyElement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Loading,
    Failed,
}

pub(super) enum Player {}

impl Player {
    pub(super) fn open(_: &Path, _: bool) -> Option<Self> {
        None
    }

    pub(super) fn state(&self) -> State {
        match *self {}
    }

    pub(super) fn size(&self) -> Option<(f32, f32)> {
        match *self {}
    }

    pub(super) fn duration(&self) -> Option<f64> {
        match *self {}
    }

    pub(super) fn position(&self) -> f64 {
        match *self {}
    }

    pub(super) fn picture(&self) -> Option<AnyElement> {
        match *self {}
    }

    pub(super) fn playing(&self) -> bool {
        match *self {}
    }

    pub(super) fn muted(&self) -> bool {
        match *self {}
    }

    pub(super) fn tick(&mut self) {
        match *self {}
    }

    pub(super) fn toggle(&mut self) {
        match *self {}
    }

    pub(super) fn set_muted(&mut self, _: bool) {
        match *self {}
    }
}
