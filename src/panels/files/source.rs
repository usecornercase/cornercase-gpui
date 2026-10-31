use std::path::{Path, PathBuf};

use super::disk::{self, Content, Stamp};
use super::served::{self, Listed};
use super::tree;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Source {
    Disk(PathBuf),
    Server(u64),
}

impl Source {
    pub(super) fn new(workspace: u64, root: PathBuf, remote: bool) -> Self {
        if remote { Self::Server(workspace) } else { Self::Disk(root) }
    }

    pub(super) fn list(&self, folders: Vec<PathBuf>) -> Listed {
        match self {
            Self::Disk(root) => folders
                .into_iter()
                .map(|rel| {
                    let listing = tree::list(root, &rel);
                    (rel, listing)
                })
                .collect(),
            Self::Server(workspace) => served::list(*workspace, folders),
        }
    }

    pub(super) fn read(&self, rel: &Path) -> (Content, Option<Stamp>) {
        match self {
            Self::Disk(root) => disk::read(&root.join(rel)),
            Self::Server(workspace) => served::read(*workspace, rel),
        }
    }

    pub(super) fn stamp(&self, rel: &Path) -> Option<Stamp> {
        match self {
            Self::Disk(root) => disk::stamp(&root.join(rel)),
            Self::Server(workspace) => served::stamp(*workspace, rel),
        }
    }
}
