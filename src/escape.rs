#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Focus {
    #[default]
    Terminal,
    Page,
    Dock,
    Elsewhere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Top {
    Choice,
    Menu,
    Pick,
    Find,
    Selection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    CancelDrag,
    CloseDialog,
    CloseSheet,
    Close(Top),
    LeaveFullScreen,
    ToPage,
    ToProgram,
    KeysBack,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "each flag is one thing the window has open")]
pub(crate) struct Scene {
    pub(crate) focus: Focus,
    pub(crate) drag: bool,
    pub(crate) dialog: bool,
    pub(crate) sheet: bool,
    pub(crate) tops: Vec<Top>,
    pub(crate) full_screen: bool,
}

impl Scene {
    pub(crate) fn step(&self) -> Step {
        let in_panel = matches!(self.focus, Focus::Page | Focus::Dock);
        if self.drag {
            Step::CancelDrag
        } else if self.dialog {
            Step::CloseDialog
        } else if self.sheet {
            Step::CloseSheet
        } else if let Some(top) = first(&self.tops).filter(|_| in_panel) {
            Step::Close(top)
        } else if self.full_screen {
            Step::LeaveFullScreen
        } else {
            match self.focus {
                Focus::Terminal => Step::ToProgram,
                Focus::Page => Step::ToPage,
                Focus::Dock | Focus::Elsewhere => Step::KeysBack,
            }
        }
    }
}

pub(crate) fn first(tops: &[Top]) -> Option<Top> {
    tops.iter().min().copied()
}

pub(crate) fn tops_of(open: &[(Top, bool)]) -> Vec<Top> {
    open.iter().filter(|(_, is_open)| *is_open).map(|(top, _)| *top).collect()
}

#[cfg(test)]
mod tests;
