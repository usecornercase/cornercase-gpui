use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{Probe, closed, shell_args, shell_path};
use crate::desktop::Lookup;
use crate::platform::{self, probe::lines_within};

const VERSION_WAIT: Duration = Duration::from_secs(5);
const SHELL_WAIT: Duration = Duration::from_secs(5);

pub(crate) struct System {
    lookup: Lookup,
}

impl System {
    pub(crate) fn here() -> Self {
        Self { lookup: Lookup::here() }
    }
}

impl Probe for System {
    fn find(&self, dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
        self.lookup.find_in(dirs, name)
    }

    fn is_program(&self, path: &Path) -> bool {
        self.lookup.is_program(path)
    }

    fn version(&self, path: &Path, path_env: Option<&str>) -> Result<Vec<String>, String> {
        let mut cmd = platform::command(path);
        cmd.arg("--version");
        if let Some(path_env) = path_env {
            cmd.env("PATH", path_env);
        }
        lines_within(&mut cmd, VERSION_WAIT, |lines| !lines.is_empty())
    }

    fn login_path(&self, shell: &Path) -> Option<String> {
        let mut cmd = platform::command(shell);
        cmd.args(shell_args(shell));
        shell_path(&lines_within(&mut cmd, SHELL_WAIT, closed).ok()?)
    }
}
