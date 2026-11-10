use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, Context, Entity, EntityId, IntoElement, ParentElement as _, Render,
    StyleRefinement, Styled as _, WeakEntity, Window, div,
};

use super::Root;
use super::status::key::StatusKey;
use crate::model::Session;
use crate::sidebar::key::SidebarKey;
use crate::sidebar::rail::key::RailKey;
use crate::tabs::key::TabsKey;
use crate::ui::counting::Counting;
use crate::ui::motion::{self, Beat};
use crate::ui::orbits::{OrbitLayer, Spots};

pub(crate) struct Drawn {
    pub(crate) element: AnyElement,
    pub(crate) counted: bool,
}

pub(crate) type Draw = fn(&mut Root, &mut Window, &mut Context<Root>) -> Drawn;

pub(crate) struct Part {
    root: WeakEntity<Root>,
    draw: Draw,
    counting: Counting,
}

impl Part {
    fn new(root: WeakEntity<Root>, draw: Draw) -> Self {
        Self { root, draw, counting: Counting::default() }
    }
}

fn counting(part: &mut Part) -> &mut Counting {
    &mut part.counting
}

impl Render for Part {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let draw = self.draw;
        let drawn = self.root.update(cx, |root, cx| draw(root, window, cx)).ok();
        if drawn.as_ref().is_some_and(|drawn| drawn.counted) {
            self.counting.drawn();
        }
        self.counting.keep(counting, cx);
        div().size_full().children(drawn.map(|drawn| drawn.element))
    }
}

pub(crate) fn cached(part: &Entity<Part>) -> AnyView {
    AnyView::from(part.clone()).cached(StyleRefinement::default().size_full())
}

pub(crate) struct Parts {
    pub(crate) sidebar: Entity<Part>,
    pub(crate) rail: Entity<Part>,
    pub(crate) tabs: Entity<Part>,
    pub(crate) status: Entity<Part>,
    pub(crate) orbit_layer: Entity<OrbitLayer>,
    pub(crate) tab_orbits: Entity<OrbitLayer>,
    pub(crate) spots: Spots,
    pub(crate) tab_spots: Spots,
    pub(crate) beat: Rc<Beat>,
    pub(crate) counted: Cell<bool>,
    pub(super) told: Told,
    pub(crate) store_seen: u64,
    pub(crate) chips_seen: u64,
}

#[derive(Default)]
pub(super) struct Told {
    sidebar: Option<SidebarKey>,
    rail: Option<RailKey>,
    tabs: Option<TabsKey>,
    status: Option<StatusKey>,
}

impl Parts {
    pub(crate) fn new(root: &WeakEntity<Root>, cx: &mut App) -> Self {
        let (spots, tab_spots) = (Spots::default(), Spots::default());
        let mut part = |draw: Draw| cx.new(|_| Part::new(root.clone(), draw));
        let (sidebar, rail) = (part(Root::sidebar_part), part(Root::rail_part));
        let (tabs, status) = (part(Root::tabs_part), part(Root::status_part));
        Self {
            sidebar,
            rail,
            tabs,
            status,
            orbit_layer: cx.new(|_| OrbitLayer::new(spots.clone())),
            tab_orbits: cx.new(|_| OrbitLayer::new(tab_spots.clone())),
            spots,
            tab_spots,
            beat: Rc::default(),
            counted: Cell::new(false),
            told: Told::default(),
            store_seen: 0,
            chips_seen: 0,
        }
    }
}

pub(crate) fn told<K: PartialEq>(last: &mut Option<K>, now: K) -> bool {
    let moved = last.as_ref() != Some(&now);
    *last = Some(now);
    moved
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Minded {
    First,
    Same,
    Missed,
}

pub(crate) fn minded<K: PartialEq>(last: &mut Option<K>, now: K) -> Minded {
    let verdict = match last.as_ref() {
        None => Minded::First,
        Some(was) if *was == now => return Minded::Same,
        Some(_) => Minded::Missed,
    };
    *last = Some(now);
    verdict
}

impl Root {
    pub(super) fn follow_parts(cx: &mut Context<Self>) {
        cx.observe_self(Self::tell_parts).detach();
        let guide = crate::tour::Guide::get(cx);
        cx.observe(&guide, |_, _, cx| cx.notify()).detach();
    }

    pub(super) fn chips_moved(&mut self, cx: &mut Context<Self>) {
        self.parts.chips_seen += 1;
        cx.notify();
    }

    fn part_ids(&self) -> [EntityId; 4] {
        let parts = &self.parts;
        [parts.sidebar.entity_id(), parts.rail.entity_id(), parts.tabs.entity_id(), parts.status.entity_id()]
    }

    fn tell_parts(&mut self, cx: &mut Context<Self>) {
        let now = (self.sidebar_key(), self.rail_key(cx), self.tabs_key(), self.status_key());
        let told_of = &mut self.parts.told;
        let moved = [
            told(&mut told_of.sidebar, now.0),
            told(&mut told_of.rail, now.1),
            told(&mut told_of.tabs, now.2),
            told(&mut told_of.status, now.3),
        ];
        for (id, moved) in self.part_ids().into_iter().zip(moved) {
            if moved {
                App::notify(cx, id);
            }
        }
    }

    pub(super) fn mind_the_parts(&mut self, window: &Window, cx: &mut Context<Self>) {
        let now = (self.sidebar_key(), self.rail_key(cx), self.tabs_key(), self.status_key());
        let told_of = &mut self.parts.told;
        let minded = [
            ("sidebar", minded(&mut told_of.sidebar, now.0)),
            ("rail", minded(&mut told_of.rail, now.1)),
            ("tab bar", minded(&mut told_of.tabs, now.2)),
            ("status bar", minded(&mut told_of.status, now.3)),
        ];
        for ((name, verdict), id) in minded.into_iter().zip(self.part_ids()) {
            if verdict == Minded::Missed {
                missed(name, id, window);
            }
        }
    }

    pub(super) fn keep_beating(&mut self, window: &Window) {
        self.parts.beat.drawing(Instant::now());
        if self.session.as_ref().is_some_and(Session::anyone_working) && self.parts.beat.start() {
            follow_frames(window, self.parts.orbit_layer.entity_id(), self.parts.beat.clone());
        }
    }
}

fn missed(name: &str, id: EntityId, window: &Window) {
    if cfg!(debug_assertions) {
        eprintln!("cornercase-gpui: the {name} changed without a notify on the window");
    }
    window.on_next_frame(move |_, cx| App::notify(cx, id));
}

fn follow_frames(window: &Window, target: EntityId, beat: Rc<Beat>) {
    window.on_next_frame(move |window, cx| match beat.tick(Instant::now(), motion::orbits_drawn()) {
        motion::Next::Wait => follow_frames(window, target, beat),
        motion::Next::Draw => cx.notify(target),
        motion::Next::Rest => {}
    });
}

#[cfg(test)]
mod tests;
