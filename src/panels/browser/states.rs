use gpui::{
    ClickEvent, Context, Div, IntoElement, ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::{Browser, Status, chrome, notice};
use crate::platform::Os;
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui;

const TEXT_WIDTH: f32 = 420.;

pub(crate) fn chrome_missing(why: &str) -> bool {
    why.contains(chrome::NO_CHROME)
}

pub(crate) fn install_hint(os: Os) -> &'static str {
    match os {
        Os::Windows => {
            "The Browser panel draws a headless Chromium, and not even Microsoft Edge was found where Windows \
             installs it. Install Google Chrome, Microsoft Edge or Brave, or set this variable to one's .exe, then \
             start cornercase-gpui again:"
        }
        Os::Mac | Os::Linux => {
            "The Browser panel draws a headless Chromium. Install Google Chrome, Chromium, Microsoft Edge or Brave, or \
             point this variable at one before starting cornercase-gpui:"
        }
    }
}

pub(crate) fn variable_hint(os: Os) -> &'static str {
    match os {
        Os::Windows => r#"setx CORNERCASE_GPUI_CHROME "C:\path\to\chrome.exe""#,
        Os::Mac | Os::Linux => "CORNERCASE_GPUI_CHROME=/path/to/chrome",
    }
}

fn explanation(text: impl Into<gpui::SharedString>) -> Div {
    div()
        .max_w(px(TEXT_WIDTH))
        .text_center()
        .text_size(px(12.))
        .line_height(px(18.))
        .text_color(colours::muted())
        .child(text.into())
}

fn code(text: &'static str) -> Div {
    div()
        .px(px(8.))
        .py(px(3.))
        .rounded(px(radius::chip()))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
        .font_family(FONT_FAMILY)
        .text_size(px(11.))
        .text_color(colours::soft())
        .child(text)
}

impl Browser {
    pub(super) fn starting() -> impl IntoElement {
        notice("Starting the browser").child(ui::busy("browser-starting", "Connecting to a headless Chrome…"))
    }

    pub(super) fn failed(why: &str, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let retry = ui::button("browser-retry", "Try again").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.attempts = 0;
            this.status = Status::Failed(String::new());
            this.restart(cx);
        }));
        if chrome_missing(why) {
            return notice("No Chromium browser found")
                .child(explanation(install_hint(Os::CURRENT)))
                .child(code(variable_hint(Os::CURRENT)))
                .child(retry);
        }
        notice("The browser cannot be shown").child(explanation(why.to_string())).child(retry)
    }
}

#[cfg(test)]
mod tests;
