use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{Bounds, MouseButton, Pixels, Point};
use serde::Deserialize;

use super::layout::{Card, Hit, Layout, rect};
use crate::model::Node;
use crate::ui::Icon;

const THRESHOLD: f32 = 5.;
const NEAR: f32 = 44.;
const HANDLE_WIDTH: f32 = 72.;
const HANDLE_TOP: f32 = 4.;
const HANDLE_HEIGHT: f32 = 10.;
const ZONE_INSET: f32 = 6.;
const ZONE_GROW: f32 = 16.;
const GHOST_SCALE: f32 = 0.42;
const GHOST_WIDTH: f32 = 300.;
const GHOST_HEIGHT: f32 = 220.;
const REVEAL: f32 = 0.7;
const LIFT: Duration = Duration::from_millis(240);
const GLIDE: Duration = Duration::from_millis(170);
const REFUSED: Duration = Duration::from_millis(520);
const SETTLE: Duration = Duration::from_millis(380);
const HOME: Duration = Duration::from_millis(300);
const GLOW: Duration = Duration::from_millis(900);
const WAIT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Swap,
    Left,
    Right,
    Above,
    Below,
}

impl Place {
    pub(crate) fn arg(self) -> &'static str {
        match self {
            Place::Swap => "swap",
            Place::Left => "left",
            Place::Right => "right",
            Place::Above => "above",
            Place::Below => "below",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Place::Swap => "Swap",
            Place::Left => "Left",
            Place::Right => "Right",
            Place::Above => "Top",
            Place::Below => "Bottom",
        }
    }

    pub(crate) fn icon(self) -> Icon {
        match self {
            Place::Swap => Icon::Swap,
            Place::Left => Icon::ArrowLeft,
            Place::Right => Icon::ArrowRight,
            Place::Above => Icon::ArrowUp,
            Place::Below => Icon::ArrowDown,
        }
    }

    pub(crate) fn zone(self, card: Bounds<Pixels>) -> Bounds<Pixels> {
        let (left, top) = (f32::from(card.origin.x), f32::from(card.origin.y));
        let (width, height) = (f32::from(card.size.width), f32::from(card.size.height));
        let part = match self {
            Place::Swap => card,
            Place::Left => rect(left, top, width / 2., height),
            Place::Right => rect(left + width / 2., top, width / 2., height),
            Place::Above => rect(left, top, width, height / 2.),
            Place::Below => rect(left, top + height / 2., width, height / 2.),
        };
        shrunk(part, ZONE_INSET)
    }
}

pub(crate) fn place_at(card: Bounds<Pixels>, at: Point<Pixels>) -> Place {
    let across = f32::from(at.x - card.origin.x) / f32::from(card.size.width).max(1.);
    let down = f32::from(at.y - card.origin.y) / f32::from(card.size.height).max(1.);
    let middle = |share: f32| (1. / 3. ..=2. / 3.).contains(&share);
    if middle(across) && middle(down) {
        return Place::Swap;
    }
    [(Place::Left, across), (Place::Right, 1. - across), (Place::Above, down), (Place::Below, 1. - down)]
        .into_iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(Place::Swap, |(place, _)| place)
}

pub(crate) fn handle(card: Bounds<Pixels>) -> Bounds<Pixels> {
    let centre = f32::from(card.origin.x) + f32::from(card.size.width) / 2.;
    rect(centre - HANDLE_WIDTH / 2., f32::from(card.origin.y) + HANDLE_TOP, HANDLE_WIDTH, HANDLE_HEIGHT)
}

pub(crate) fn near_top(layout: &Layout, at: Point<Pixels>) -> Option<u64> {
    if layout.cards.len() < 2 {
        return None;
    }
    let card = layout.cards.iter().find(|card| card.bounds.contains(&at))?;
    (f32::from(at.y - card.bounds.origin.y) < NEAR).then_some(card.pane)
}

pub(crate) fn shrunk(area: Bounds<Pixels>, by: f32) -> Bounds<Pixels> {
    let (width, height) = (f32::from(area.size.width), f32::from(area.size.height));
    let (across, down) = (by.min(width / 2.), by.min(height / 2.));
    rect(f32::from(area.origin.x) + across, f32::from(area.origin.y) + down, width - across * 2., height - down * 2.)
}

fn quint(t: f32) -> f32 {
    1. - (1. - t).powi(5)
}

fn progress(since: Instant, now: Instant, length: Duration) -> f32 {
    (now.saturating_duration_since(since).as_secs_f32() / length.as_secs_f32()).clamp(0., 1.)
}

fn mix(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn after(t: f32, start: f32) -> f32 {
    ((t - start) / (1. - start)).clamp(0., 1.)
}

pub(crate) fn lerp(from: Bounds<Pixels>, to: Bounds<Pixels>, t: f32) -> Bounds<Pixels> {
    let at = |a: Pixels, b: Pixels| mix(f32::from(a), f32::from(b), t);
    rect(
        at(from.origin.x, to.origin.x),
        at(from.origin.y, to.origin.y),
        at(from.size.width, to.size.width),
        at(from.size.height, to.size.height),
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Shown {
    pub rect: Bounds<Pixels>,
    pub alpha: f32,
}

#[derive(Debug, Clone, Copy)]
struct Glide {
    from: Shown,
    to: Shown,
    start: Instant,
    length: Duration,
}

impl Glide {
    fn at(&self, now: Instant) -> Shown {
        let t = quint(progress(self.start, now, self.length));
        Shown { rect: lerp(self.from.rect, self.to.rect, t), alpha: mix(self.from.alpha, self.to.alpha, t) }
    }

    fn done(&self, now: Instant) -> bool {
        progress(self.start, now, self.length) >= 1.
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Aim {
    pub target: u64,
    pub place: Place,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ZoneLook {
    pub shown: Shown,
    pub place: Place,
    pub refused: bool,
}

#[derive(Debug, Clone, Copy)]
struct Zone {
    aim: Aim,
    glide: Glide,
    open: bool,
    refused: bool,
}

impl Zone {
    fn open(aim: Aim, area: Bounds<Pixels>, now: Instant) -> Self {
        let from = Shown { rect: shrunk(area, ZONE_GROW), alpha: 0. };
        let glide = Glide { from, to: Shown { rect: area, alpha: 1. }, start: now, length: GLIDE };
        Self { aim, glide, open: true, refused: false }
    }

    fn retarget(&mut self, aim: Aim, area: Bounds<Pixels>, now: Instant) {
        let to = Shown { rect: area, alpha: 1. };
        self.glide = Glide { from: self.glide.at(now), to, start: now, length: GLIDE };
        self.aim = aim;
        self.open = true;
    }

    fn close(&mut self, now: Instant) {
        if !self.open {
            return;
        }
        let here = self.glide.at(now);
        let to = Shown { rect: shrunk(here.rect, ZONE_GROW / 2.), alpha: 0. };
        let length = if self.refused { REFUSED } else { GLIDE };
        self.glide = Glide { from: here, to, start: now, length };
        self.open = false;
    }

    fn look(&self, now: Instant) -> ZoneLook {
        ZoneLook { shown: self.glide.at(now), place: self.aim.place, refused: self.refused }
    }

    fn gone(&self, now: Instant) -> bool {
        !self.open && self.glide.done(now)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grip {
    Handle,
    Right,
}

impl Grip {
    pub(crate) fn button(self) -> MouseButton {
        match self {
            Grip::Handle => MouseButton::Left,
            Grip::Right => MouseButton::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Press {
    pub pane: u64,
    pub grip: Grip,
    pub cell: (u16, u16),
    pub at: Point<Pixels>,
}

#[derive(Debug, Clone, Copy)]
struct Lift {
    press: Press,
    at: Point<Pixels>,
    lifted: Option<(Instant, Bounds<Pixels>)>,
    zone: Option<Zone>,
}

impl Lift {
    fn moved(&self) -> f32 {
        let (x, y) = (f32::from(self.at.x - self.press.at.x), f32::from(self.at.y - self.press.at.y));
        x.hypot(y)
    }

    fn lift_off(&mut self, layout: &Layout, now: Instant) {
        if self.lifted.is_some() || layout.cards.len() < 2 {
            return;
        }
        let Some(home) = layout.card(self.press.pane).map(|card| card.bounds) else { return };
        let away = match self.press.grip {
            Grip::Handle => self.moved() >= THRESHOLD,
            Grip::Right => !home.contains(&self.at),
        };
        if away {
            self.lifted = Some((now, home));
        }
    }

    fn aim(&self, layout: &Layout) -> Option<Aim> {
        let before = self.zone.filter(|zone| zone.open).map(|zone| zone.aim);
        match layout.hit(self.at) {
            Some(Hit::Pane(target, _)) if target != self.press.pane => {
                let card = layout.card(target)?;
                Some(Aim { target, place: place_at(card.bounds, self.at) })
            }
            Some(Hit::Gap(_)) => before,
            _ => None,
        }
    }

    fn steer(&mut self, layout: &Layout, now: Instant) {
        let aim = self.aim(layout);
        if aim == self.zone.filter(|zone| zone.open).map(|zone| zone.aim) {
            return;
        }
        let area = aim.and_then(|aim| layout.card(aim.target).map(|card| aim.place.zone(card.bounds)));
        match (aim.zip(area), self.zone.as_mut()) {
            (Some((aim, area)), Some(zone)) => zone.retarget(aim, area, now),
            (Some((aim, area)), None) => self.zone = Some(Zone::open(aim, area, now)),
            (None, Some(zone)) => zone.close(now),
            (None, None) => {}
        }
    }

    fn ghost(&self, now: Instant) -> Option<Ghost> {
        let (since, home) = self.lifted?;
        let scale = ghost_scale(home);
        let t = quint(progress(since, now, LIFT));
        let rect = lerp(home, hanging(home, self.at, scale), t);
        Some(Ghost { pane: self.press.pane, rect, scale: mix(1., scale, t), alpha: 1., zone: None })
    }
}

fn ghost_scale(home: Bounds<Pixels>) -> f32 {
    let (width, height) = (f32::from(home.size.width).max(1.), f32::from(home.size.height).max(1.));
    GHOST_SCALE.min(GHOST_WIDTH / width).min(GHOST_HEIGHT / height)
}

fn hanging(home: Bounds<Pixels>, at: Point<Pixels>, scale: f32) -> Bounds<Pixels> {
    let (width, height) = (f32::from(home.size.width) * scale, f32::from(home.size.height) * scale);
    let grip = (HANDLE_TOP + HANDLE_HEIGHT / 2.) * scale;
    rect(f32::from(at.x) - width / 2., f32::from(at.y) - grip, width, height)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ghost {
    pub pane: u64,
    pub rect: Bounds<Pixels>,
    pub scale: f32,
    pub alpha: f32,
    pub zone: Option<ZoneLook>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Toward {
    Wait,
    Card,
    Home,
}

#[derive(Debug, Clone)]
struct Flight {
    pane: u64,
    from: Bounds<Pixels>,
    scale: f32,
    toward: Toward,
    start: Instant,
    tree: Option<Node>,
}

impl Flight {
    fn progress(&self, now: Instant) -> f32 {
        match self.toward {
            Toward::Wait => 0.,
            Toward::Card => progress(self.start, now, SETTLE),
            Toward::Home => progress(self.start, now, HOME),
        }
    }

    fn ghost(&self, card: Bounds<Pixels>, now: Instant) -> Ghost {
        let t = self.progress(now);
        let eased = quint(t);
        Ghost {
            pane: self.pane,
            rect: lerp(self.from, card, eased),
            scale: mix(self.scale, 1., eased),
            alpha: 1. - after(t, REVEAL),
            zone: None,
        }
    }

    fn turn(&mut self, toward: Toward, now: Instant) {
        self.toward = toward;
        self.start = now;
    }
}

#[derive(Debug, Clone)]
struct Expected {
    pane: u64,
    tree: Option<Node>,
    since: Instant,
}

#[derive(Debug, Clone)]
struct Shuffle {
    from: HashMap<u64, Bounds<Pixels>>,
    start: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Release {
    Nothing,
    Click(Press),
    Drop(u64, Aim),
    Home,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Answer {
    Refused,
    Moved(Option<Node>),
}

#[derive(Deserialize)]
struct Moved {
    layout: Node,
}

impl Answer {
    pub(crate) fn read(result: &anyhow::Result<String>) -> Self {
        match result {
            Err(_) => Self::Refused,
            Ok(out) => Self::Moved(serde_json::from_str::<Moved>(out).ok().map(|moved| moved.layout)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Look {
    pub shown: f32,
    pub hollow: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Stage {
    pub hollow: Option<(u64, f32)>,
    pub hidden: Option<(u64, f32)>,
    pub cards: Vec<(u64, Bounds<Pixels>)>,
    pub zone: Option<ZoneLook>,
    pub glow: Option<(u64, f32)>,
}

impl Stage {
    pub(crate) fn shown(&self, card: &Card) -> (Card, Look) {
        let bounds = self.cards.iter().find(|(pane, _)| *pane == card.pane).map_or(card.bounds, |(_, at)| *at);
        let cells = bounds.origin + (card.cells - card.bounds.origin);
        let hollow = self.hollow.filter(|(pane, _)| *pane == card.pane).map_or(0., |(_, by)| by);
        let shown = self.hidden.filter(|(pane, _)| *pane == card.pane).map_or(1., |(_, by)| by);
        (Card { bounds, cells, ..*card }, Look { shown, hollow })
    }
}

#[derive(Debug, Default)]
pub(crate) struct Moves {
    near: Option<u64>,
    lift: Option<Lift>,
    flight: Option<Flight>,
    expected: Option<Expected>,
    zone: Option<Zone>,
    shuffle: Option<Shuffle>,
    glow: Option<(u64, Instant)>,
}

impl Moves {
    pub(crate) fn near(&self) -> Option<u64> {
        self.near.filter(|_| self.lift.is_none() && self.flight.is_none())
    }

    pub(crate) fn hover(&mut self, near: Option<u64>) -> bool {
        let changed = self.near != near;
        self.near = near;
        changed
    }

    pub(crate) fn press(&mut self, press: Press) {
        self.near = None;
        self.lift = Some(Lift { press, at: press.at, lifted: None, zone: None });
    }

    pub(crate) fn pressed(&self) -> Option<Press> {
        self.lift.map(|lift| lift.press)
    }

    pub(crate) fn lifted(&self) -> bool {
        self.lift.is_some_and(|lift| lift.lifted.is_some())
    }

    pub(crate) fn flying(&self) -> bool {
        self.lifted() || self.flight.is_some()
    }

    pub(crate) fn drag(&mut self, at: Point<Pixels>, layout: &Layout, now: Instant) {
        let Some(lift) = self.lift.as_mut() else { return };
        lift.at = at;
        lift.lift_off(layout, now);
        if lift.lifted.is_some() {
            lift.steer(layout, now);
        }
    }

    pub(crate) fn release(&mut self, tree: Option<Node>, now: Instant) -> Release {
        let Some(lift) = self.lift.take() else { return Release::Nothing };
        let Some(ghost) = lift.ghost(now) else { return Release::Click(lift.press) };
        let aim = lift.zone.filter(|zone| zone.open).map(|zone| zone.aim);
        let toward = if aim.is_some() { Toward::Wait } else { Toward::Home };
        let (pane, from, scale) = (lift.press.pane, ghost.rect, ghost.scale);
        let left_home = aim.is_some() || scale < 1.;
        self.flight = left_home.then_some(Flight { pane, from, scale, toward, start: now, tree });
        self.zone = lift.zone;
        aim.map_or(Release::Home, |aim| Release::Drop(pane, aim))
    }

    pub(crate) fn cancel(&mut self, now: Instant) -> bool {
        let Some(lift) = self.lift.as_mut() else { return false };
        if let Some(zone) = lift.zone.as_mut() {
            zone.close(now);
        }
        let _ = self.release(None, now);
        true
    }

    pub(crate) fn expect(&mut self, pane: u64, tree: Option<Node>, now: Instant) {
        self.expected = Some(Expected { pane, tree, since: now });
    }

    pub(crate) fn answered(&mut self, answer: &Answer, now: Instant) {
        let unchanged = |tree: Option<&Node>| matches!(answer, Answer::Moved(Some(moved)) if tree == Some(moved));
        let refused = *answer == Answer::Refused;
        if self.expected.as_ref().is_some_and(|expected| refused || unchanged(expected.tree.as_ref())) {
            self.expected = None;
        }
        let Some(flight) = self.flight.as_mut().filter(|flight| flight.toward == Toward::Wait) else { return };
        if refused || unchanged(flight.tree.as_ref()) {
            flight.turn(Toward::Home, now);
            self.close_zone(refused, now);
        }
    }

    pub(crate) fn laid_out(&mut self, before: &Layout, after: &Layout, now: Instant) {
        if before.cards.is_empty() || before.cards == after.cards {
            return;
        }
        if let Some(flight) = self.flight.as_mut().filter(|flight| flight.toward == Toward::Wait) {
            flight.turn(Toward::Card, now);
            let pane = flight.pane;
            self.shuffle = Some(shuffle_from(before, Some(pane), now));
            self.close_zone(false, now);
        } else if let Some(expected) = self.expected.take() {
            self.shuffle = Some(shuffle_from(before, None, now));
            self.glow = Some((expected.pane, now));
        }
    }

    fn close_zone(&mut self, refused: bool, now: Instant) {
        if let Some(zone) = self.zone.as_mut() {
            zone.refused |= refused;
            zone.close(now);
        }
    }

    pub(crate) fn tick(&mut self, now: Instant) {
        self.land(now);
        self.zone = self.zone.filter(|zone| !zone.gone(now));
        if let Some(lift) = self.lift.as_mut() {
            lift.zone = lift.zone.filter(|zone| !zone.gone(now));
        }
        self.shuffle = self.shuffle.take().filter(|shuffle| progress(shuffle.start, now, SETTLE) < 1.);
        self.glow = self.glow.filter(|(_, since)| progress(*since, now, GLOW) < 1.);
        self.expected = self.expected.take().filter(|expected| now.saturating_duration_since(expected.since) < WAIT);
    }

    fn land(&mut self, now: Instant) {
        let Some(flight) = self.flight.as_mut() else { return };
        match flight.toward {
            Toward::Wait if now.saturating_duration_since(flight.start) >= WAIT => {
                flight.turn(Toward::Home, now);
                self.close_zone(false, now);
            }
            Toward::Card if flight.progress(now) >= 1. => {
                self.glow = Some((flight.pane, now));
                self.flight = None;
            }
            Toward::Home if flight.progress(now) >= 1. => self.flight = None,
            _ => {}
        }
    }

    pub(crate) fn busy(&self, now: Instant) -> bool {
        let lifting = self.lift.is_some_and(|lift| {
            lift.lifted.is_some_and(|(since, _)| progress(since, now, LIFT) < 1.)
                || lift.zone.is_some_and(|zone| !zone.glide.done(now))
        });
        lifting || self.flight.is_some() || self.zone.is_some() || self.shuffle.is_some() || self.glow.is_some()
    }

    pub(crate) fn stage(&self, layout: &Layout, now: Instant) -> Stage {
        Stage {
            hollow: self.hollow(now),
            hidden: self.hidden(now),
            cards: self.shuffled(layout, now),
            zone: self.zone_look(now),
            glow: self.glow.map(|(pane, since)| (pane, (1. - progress(since, now, GLOW)).powi(2))),
        }
    }

    fn hollow(&self, now: Instant) -> Option<(u64, f32)> {
        if let Some((pane, (since, _))) = self.lift.and_then(|lift| lift.lifted.map(|lifted| (lift.press.pane, lifted)))
        {
            return Some((pane, quint(progress(since, now, LIFT))));
        }
        let flight = self.flight.as_ref()?;
        match flight.toward {
            Toward::Wait => Some((flight.pane, 1.)),
            Toward::Home => Some((flight.pane, 1. - after(flight.progress(now), REVEAL))),
            Toward::Card => None,
        }
    }

    fn hidden(&self, now: Instant) -> Option<(u64, f32)> {
        let flight = self.flight.as_ref().filter(|flight| flight.toward == Toward::Card)?;
        Some((flight.pane, after(flight.progress(now), REVEAL)))
    }

    fn shuffled(&self, layout: &Layout, now: Instant) -> Vec<(u64, Bounds<Pixels>)> {
        let Some(shuffle) = &self.shuffle else { return Vec::new() };
        let t = quint(progress(shuffle.start, now, SETTLE));
        let slid = |card: &Card| shuffle.from.get(&card.pane).map(|from| (card.pane, lerp(*from, card.bounds, t)));
        layout.cards.iter().filter_map(slid).collect()
    }

    fn zone_look(&self, now: Instant) -> Option<ZoneLook> {
        self.lift.and_then(|lift| lift.zone).or(self.zone).map(|zone| zone.look(now))
    }

    pub(crate) fn ghost(&self, layout: &Layout, now: Instant) -> Option<Ghost> {
        let ghost = match (self.lift, self.flight.as_ref()) {
            (Some(lift), _) => lift.ghost(now)?,
            (None, Some(flight)) => flight.ghost(layout.card(flight.pane)?.bounds, now),
            (None, None) => return None,
        };
        Some(Ghost { zone: self.zone_look(now), ..ghost })
    }
}

fn shuffle_from(before: &Layout, flying: Option<u64>, now: Instant) -> Shuffle {
    let from = before.cards.iter().filter(|card| Some(card.pane) != flying).map(|card| (card.pane, card.bounds));
    Shuffle { from: from.collect(), start: now }
}

#[cfg(test)]
mod tests;
