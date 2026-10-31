mod body;
mod card;
mod place;
mod player;
mod quicklook;

use std::time::Duration;

use gpui::{
    AnyView, App, AppContext as _, Bounds, Context, Entity, Global, Pixels, Size, Task, UniformListScrollHandle,
    WeakEntity,
};

use super::FilesPanel;
use super::viewer::show_link;
use crate::model::Link;
use crate::store::{Store, ToastKind};
use body::Body;

const PEEK_AFTER: Duration = Duration::from_millis(350);
const GRACE: Duration = Duration::from_millis(280);

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Anchor {
    pub link: Bounds<Pixels>,
    pub room: Size<Pixels>,
}

pub(crate) struct Peek {
    link: Option<Link>,
    anchor: Anchor,
    files: WeakEntity<FilesPanel>,
    body: Option<Body>,
    over_link: bool,
    over_card: bool,
    generation: u64,
    timer: Option<Task<()>>,
    code_scroll: UniformListScrollHandle,
}

struct GlobalPeek(Entity<Peek>);

impl Global for GlobalPeek {}

impl Peek {
    pub(crate) fn get(cx: &mut App) -> Entity<Peek> {
        if let Some(GlobalPeek(peek)) = cx.try_global::<GlobalPeek>() {
            return peek.clone();
        }
        let peek = cx.new(|_| Peek::new());
        cx.set_global(GlobalPeek(peek.clone()));
        peek
    }

    fn new() -> Self {
        Self {
            link: None,
            anchor: Anchor::default(),
            files: WeakEntity::new_invalid(),
            body: None,
            over_link: false,
            over_card: false,
            generation: 0,
            timer: None,
            code_scroll: UniformListScrollHandle::new(),
        }
    }

    pub(crate) fn hover(&mut self, link: Link, anchor: Anchor, files: &AnyView, cx: &mut Context<Self>) {
        self.over_link = true;
        self.anchor = anchor;
        if let Ok(panel) = files.clone().downcast::<FilesPanel>() {
            self.files = panel.downgrade();
        }
        if self.link.as_ref().is_some_and(|shown| shown.same_file(&link)) {
            self.link = Some(link);
            cx.notify();
            return;
        }
        self.generation += 1;
        self.link = Some(link);
        self.body = None;
        self.later(PEEK_AFTER, Self::open, cx);
        cx.notify();
    }

    pub(crate) fn left_link(&mut self, cx: &mut Context<Self>) {
        self.over_link = false;
        self.settle(cx);
    }

    pub(crate) fn close(&mut self, cx: &mut Context<Self>) {
        if self.link.is_none() {
            return;
        }
        self.generation += 1;
        self.link = None;
        self.body = None;
        self.timer = None;
        self.over_link = false;
        self.over_card = false;
        cx.notify();
    }

    fn card_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.over_card = hovered;
        self.settle(cx);
    }

    fn settle(&mut self, cx: &mut Context<Self>) {
        if self.over_link || self.over_card {
            return;
        }
        if self.body.is_none() {
            self.close(cx);
            return;
        }
        self.later(GRACE, Self::close_if_left, cx);
    }

    fn close_if_left(&mut self, cx: &mut Context<Self>) {
        if !self.over_link && !self.over_card {
            self.close(cx);
        }
    }

    fn later(&mut self, after: Duration, then: fn(&mut Self, &mut Context<Self>), cx: &mut Context<Self>) {
        let generation = self.generation;
        self.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(after).await;
            let _ = this.update(cx, |this, cx| this.fire(generation, then, cx));
        }));
    }

    fn fire(&mut self, generation: u64, then: fn(&mut Self, &mut Context<Self>), cx: &mut Context<Self>) {
        if self.generation == generation {
            then(self, cx);
        }
    }

    fn open(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        self.body = Some(Body::Loading);
        self.load(&link, cx);
        cx.notify();
    }

    fn open_in_files(&mut self, at: Option<usize>, cx: &mut Context<Self>) {
        let (Some(link), files) = (self.link.clone(), self.files.clone()) else { return };
        self.close(cx);
        show_link(&files, &link, at, cx);
    }

    fn open_in_app(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.link.as_ref().map(|link| link.path.clone()) else { return };
        self.close(cx);
        if let Err(why) = std::process::Command::new("open").arg(&path).spawn() {
            let text = format!("Could not open {path}: {why}");
            Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Error, cx));
        }
    }
}
