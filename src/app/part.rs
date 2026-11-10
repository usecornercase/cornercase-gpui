use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    StyleRefinement, Styled as _, WeakEntity, Window, div,
};

use super::Root;
use crate::sidebar::key::SidebarKey;
use crate::ui::counting::Counting;
use crate::ui::motion::Beat;
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
    pub(crate) orbit_layer: Entity<OrbitLayer>,
    pub(crate) spots: Spots,
    pub(crate) beat: Rc<Beat>,
    pub(crate) counted: Cell<bool>,
    pub(super) told: Option<SidebarKey>,
    pub(crate) store_seen: u64,
}

impl Parts {
    pub(crate) fn new(root: &WeakEntity<Root>, sidebar: Draw, cx: &mut App) -> Self {
        let spots = Spots::default();
        Self {
            sidebar: cx.new(|_| Part::new(root.clone(), sidebar)),
            orbit_layer: cx.new(|_| OrbitLayer::new(spots.clone())),
            spots,
            beat: Rc::default(),
            counted: Cell::new(false),
            told: None,
            store_seen: 0,
        }
    }
}

pub(crate) fn sidebar_told(before: Option<&SidebarKey>, now: &SidebarKey) -> bool {
    before != Some(now)
}

impl Root {
    pub(super) fn tell_parts(&mut self, cx: &mut Context<Self>) {
        let now = self.sidebar_key();
        let told = sidebar_told(self.parts.told.as_ref(), &now);
        self.parts.told = Some(now);
        if told {
            self.parts.sidebar.update(cx, |_, cx| cx.notify());
        }
    }

    pub(super) fn mind_the_sidebar(&mut self, window: &Window) {
        let now = self.sidebar_key();
        let Some(told) = self.parts.told.as_ref() else {
            self.parts.told = Some(now);
            return;
        };
        if *told == now {
            return;
        }
        if cfg!(debug_assertions) {
            eprintln!("cornercase-gpui: the sidebar changed without a notify on the window");
        }
        self.parts.told = Some(now);
        let sidebar = self.parts.sidebar.entity_id();
        window.on_next_frame(move |_, cx| cx.notify(sidebar));
    }
}

#[cfg(test)]
mod tests;
