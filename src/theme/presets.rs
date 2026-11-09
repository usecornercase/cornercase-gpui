macro_rules! preset {
    ($file:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/themes/", $file, ".json"))
    };
}

pub(crate) const PRESETS: &[(&str, &str)] = &[
    ("cornercase-dark", preset!("cornercase-dark")),
    ("cornercase-light", preset!("cornercase-light")),
    ("tokyo-night", preset!("tokyo-night")),
    ("catppuccin-mocha", preset!("catppuccin-mocha")),
    ("catppuccin-latte", preset!("catppuccin-latte")),
    ("dracula", preset!("dracula")),
    ("gruvbox-dark", preset!("gruvbox-dark")),
    ("nord", preset!("nord")),
    ("rose-pine", preset!("rose-pine")),
    ("solarized-dark", preset!("solarized-dark")),
    ("solarized-light", preset!("solarized-light")),
];

pub(crate) fn text(id: &str) -> Option<&'static str> {
    PRESETS.iter().find(|(preset, _)| *preset == id).map(|(_, text)| *text)
}

pub(crate) fn is_preset(id: &str) -> bool {
    text(id).is_some()
}

#[cfg(test)]
mod tests;
