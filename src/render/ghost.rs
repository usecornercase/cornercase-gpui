use gpui::{
    App, Background, BorderStyle, Bounds, BoxShadow, ContentMask, FontWeight, Hsla, Pixels, Rgba, ShapedLine,
    SharedString, TransformationMatrix, Window, fill, font, point, px, quad, size, transparent_black,
};

use super::layout::{Layout, PADDING};
use super::moving::{Ghost, Stage, ZoneLook, shrunk};
use super::{Metrics, PaneCanvas, text_run};
use crate::model::PaneGrid;
use crate::theme::{Painter, colours, faded, hsla, radius};
use crate::ui::glass;

const CAPTION_HEIGHT: f32 = 28.;
const CAPTION_ICON: f32 = 14.;
const CAPTION_GAP: f32 = 6.;
const CAPTION_SIDES: f32 = 13.;
const CAPTION_SPACE: f32 = 10.;
const GHOST_GROUND: f32 = 0.95;

fn white(alpha: f32) -> Hsla {
    faded(colours::on_brand().into(), alpha)
}

fn tinted(colour: Rgba, alpha: f32) -> Hsla {
    faded(colour.into(), alpha)
}

fn lifted(alpha: f32) -> Vec<BoxShadow> {
    let fading = |shadow: BoxShadow| BoxShadow { color: shadow.color.opacity(alpha), ..shadow };
    let mut shadows: Vec<BoxShadow> = glass::lift().into_iter().map(fading).collect();
    shadows.extend(glass::glow(colours::brand(), 0.45 * alpha, 22.));
    shadows
}

pub(crate) fn paint_ghost(pane: &PaneGrid, ghost: &Ghost, metrics: &Metrics, window: &mut Window, cx: &mut App) {
    let corner = px(radius::CARD);
    window.paint_shadows(ghost.rect, corner.into(), &lifted(ghost.alpha));
    let ground = hsla(Painter { colours: &pane.colours }.default_bg());
    window.paint_quad(fill(ghost.rect, faded(ground, GHOST_GROUND * ghost.alpha)).corner_radii(corner));
    paint_ghost_text(pane, ghost, metrics, window, cx);
    let edge = tinted(colours::accent(), 0.9 * ghost.alpha);
    window.paint_quad(quad(ghost.rect, corner, transparent_black(), px(1.), edge, BorderStyle::Solid));
    let centre = ghost.rect.center().x;
    let grip = Bounds::new(point(centre - px(13.), ghost.rect.origin.y + px(4.)), size(px(26.), px(3.)));
    window.paint_quad(fill(grip, white(0.6 * ghost.alpha)).corner_radii(px(1.5)));
    if let Some(zone) = &ghost.zone {
        paint_caption(ghost, zone, window, cx);
    }
}

fn paint_ghost_text(pane: &PaneGrid, ghost: &Ghost, metrics: &Metrics, window: &mut Window, cx: &mut App) {
    let scaled = metrics.scaled(ghost.scale);
    let inner = shrunk(ghost.rect, PADDING * ghost.scale);
    let canvas = PaneCanvas::plain(pane, &scaled, inner.origin, ghost.alpha);
    window.with_content_mask(Some(ContentMask { bounds: inner }), |window| canvas.paint_rows(pane, window, cx));
}

pub(super) fn paint_marks(stage: &Stage, layout: &Layout, window: &mut Window) {
    if let Some(zone) = &stage.zone {
        paint_zone(zone, window);
    }
    let glowing = stage.glow.and_then(|(pane, strength)| Some((layout.card(pane)?, strength)));
    if let Some((card, strength)) = glowing {
        paint_glow(stage.shown(card).0.bounds, strength, window);
    }
}

fn tones(zone: &ZoneLook) -> (Rgba, Rgba) {
    if zone.refused { (colours::failed(), colours::failed()) } else { (colours::brand(), colours::accent()) }
}

fn paint_zone(zone: &ZoneLook, window: &mut Window) {
    let alpha = zone.shown.alpha;
    if alpha <= 0.01 {
        return;
    }
    let (wash, edge) = tones(zone);
    let corner = px(radius::CONTROL);
    let border = tinted(edge, 0.85 * alpha);
    window.paint_quad(quad(zone.shown.rect, corner, tinted(wash, 0.17 * alpha), px(1.5), border, BorderStyle::Solid));
}

fn label(text: &'static str, colour: Hsla, window: &mut Window) -> ShapedLine {
    let mut face = font(".SystemUIFont");
    face.weight = FontWeight::SEMIBOLD;
    let runs = [text_run(text.len(), face, colour)];
    window.text_system().shape_line(SharedString::from(text), px(12.), &runs, None)
}

fn caption_at(ghost: &Ghost, width: f32, window: &Window) -> Bounds<Pixels> {
    let room = window.viewport_size();
    let below = ghost.rect.bottom() + px(CAPTION_SPACE);
    let lowest = room.height - px(CAPTION_SPACE + CAPTION_HEIGHT);
    let top = if below > lowest { ghost.rect.origin.y - px(CAPTION_SPACE + CAPTION_HEIGHT) } else { below };
    let widest = f32::from(room.width) - width - CAPTION_SPACE;
    let left = (f32::from(ghost.rect.center().x) - width / 2.).min(widest).max(CAPTION_SPACE);
    Bounds::new(point(px(left), top), size(px(width), px(CAPTION_HEIGHT)))
}

fn paint_caption(ghost: &Ghost, zone: &ZoneLook, window: &mut Window, cx: &mut App) {
    let alpha = zone.shown.alpha * ghost.alpha;
    if alpha <= 0.01 {
        return;
    }
    let text = label(zone.place.label(), white(alpha), window);
    let caption = caption_at(ghost, CAPTION_SIDES * 2. + CAPTION_ICON + CAPTION_GAP + f32::from(text.width), window);
    let (tone, _) = tones(zone);
    let corner = px(CAPTION_HEIGHT / 2.);
    window.paint_shadows(caption, corner.into(), &glass::glow(tone, 0.55 * alpha, 16.));
    let ground: Background = if zone.refused { tinted(tone, alpha).into() } else { glass::cobalt().opacity(alpha) };
    window.paint_quad(fill(caption, ground).corner_radii(corner));
    let icon = Bounds::new(
        point(caption.origin.x + px(CAPTION_SIDES), caption.center().y - px(CAPTION_ICON / 2.)),
        size(px(CAPTION_ICON), px(CAPTION_ICON)),
    );
    let path = SharedString::from(zone.place.icon().path());
    let _ = window.paint_svg(icon, path, TransformationMatrix::unit(), white(alpha), cx);
    let _ = text.paint(point(icon.right() + px(CAPTION_GAP), caption.origin.y), px(CAPTION_HEIGHT), window, cx);
}

fn paint_glow(card: Bounds<Pixels>, strength: f32, window: &mut Window) {
    let ring = card.dilate(px(3.));
    let edge = tinted(colours::accent(), 0.9 * strength);
    window.paint_quad(quad(ring, px(radius::CARD + 3.), transparent_black(), px(2.), edge, BorderStyle::Solid));
    window.paint_quad(fill(card, tinted(colours::brand(), 0.1 * strength)).corner_radii(px(radius::CARD)));
}
