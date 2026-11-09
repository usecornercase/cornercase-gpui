use std::sync::{Arc, LazyLock};

use parking_lot::RwLock;

use super::presets;
use super::resolve::{Lookup, ROOT, Theme, resolve};

const LEAST_OPACITY: f32 = 0.3;

pub(crate) struct Presets;

impl Lookup for Presets {
    fn text(&self, id: &str) -> Option<String> {
        presets::text(id).map(str::to_string)
    }
}

pub(crate) fn preset(id: &str) -> Option<Theme> {
    resolve(id, &Presets).ok().map(|resolved| resolved.theme)
}

pub(crate) fn fallback() -> Theme {
    let found = preset(ROOT);
    found.expect("the built-in theme resolves")
}

static CURRENT: LazyLock<RwLock<Arc<Theme>>> = LazyLock::new(|| RwLock::new(Arc::new(fallback())));

static OPACITY_OVERRIDE: LazyLock<Option<f32>> =
    LazyLock::new(|| opacity_from(std::env::var("CORNERCASE_GPUI_OPACITY").ok().as_deref()));

pub(crate) fn current() -> Arc<Theme> {
    CURRENT.read().clone()
}

pub(crate) fn read<T>(pick: impl FnOnce(&Theme) -> T) -> T {
    pick(&CURRENT.read())
}

pub(crate) fn set(theme: Theme) {
    *CURRENT.write() = Arc::new(theme);
}

pub(crate) fn opacity_from(value: Option<&str>) -> Option<f32> {
    value.and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite()).map(|v| v.clamp(LEAST_OPACITY, 1.))
}

pub(crate) fn opacity_override() -> Option<f32> {
    *OPACITY_OVERRIDE
}
