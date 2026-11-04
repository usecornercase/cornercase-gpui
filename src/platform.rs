#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Os {
    Mac,
    Linux,
}

impl Os {
    pub(crate) const CURRENT: Os = if cfg!(target_os = "macos") { Os::Mac } else { Os::Linux };
}
