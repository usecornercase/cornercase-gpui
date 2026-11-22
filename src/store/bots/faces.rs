use std::path::PathBuf;

use gpui::Context;

use super::super::{Store, ToastKind};
use crate::model::bots::{BotKey, Faces};
use crate::prefs;

const FILE: &str = "faces.json";

fn faces_path() -> PathBuf {
    prefs::beside(&prefs::path(), FILE)
}

pub(crate) fn load_faces() -> Faces {
    std::fs::read_to_string(faces_path()).map(|text| Faces::parse(&text)).unwrap_or_default()
}

impl Store {
    pub(crate) fn face_of(&self, key: &BotKey) -> u8 {
        self.bots.faces.of(key)
    }

    pub(crate) fn set_face(&mut self, key: &BotKey, colour: u8, cx: &mut Context<Self>) {
        if self.bots.faces.set(key, colour) {
            self.keep_faces(cx);
        }
    }

    pub(crate) fn keep_picked_face(&mut self, picked: Option<(BotKey, u8)>, cx: &mut Context<Self>) {
        if let Some((key, colour)) = picked {
            self.set_face(&key, colour, cx);
        }
    }

    pub(super) fn forget_face(&mut self, key: &BotKey, cx: &mut Context<Self>) {
        if self.bots.faces.forget(key) {
            self.keep_faces(cx);
        }
    }

    fn keep_faces(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = prefs::write_atomic(&faces_path(), &self.bots.faces.text()) {
            self.toast(format!("Could not keep the bots' faces: {error:#}"), ToastKind::Error, cx);
        }
        cx.notify();
    }
}
