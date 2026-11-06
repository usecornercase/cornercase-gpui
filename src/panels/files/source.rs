use std::path::{Path, PathBuf};

use super::disk::{self, Content, Stamp};
use super::gutter::Gutter;
use super::served::{self, Listed};
use super::there::There;
use super::{Mode, Results, tree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Source {
    Disk { root: PathBuf, workspace: u64 },
    Server(u64),
    There(There),
}

impl Source {
    pub(super) fn new(workspace: u64, root: PathBuf, remote: bool) -> Self {
        if remote { Self::Server(workspace) } else { Self::Disk { root, workspace } }
    }

    pub(super) fn lists_marks(&self) -> bool {
        matches!(self, Self::Server(_) | Self::There(_))
    }

    pub(super) fn marks(&self, folders: Vec<PathBuf>) -> Listed {
        match self {
            Self::Disk { workspace, .. } | Self::Server(workspace) => served::list(*workspace, folders),
            Self::There(there) => there.list(folders),
        }
    }

    pub(super) fn gutter(&self, rel: &Path) -> Option<Gutter> {
        match self {
            Self::Disk { workspace, .. } | Self::Server(workspace) => served::gutter(*workspace, rel),
            Self::There(there) => there.gutter(rel),
        }
    }

    pub(super) fn on_this_disk(&self) -> bool {
        matches!(self, Self::Disk { .. })
    }

    pub(super) fn is_there(&self) -> bool {
        matches!(self, Self::There(_))
    }

    pub(super) fn list(&self, folders: Vec<PathBuf>) -> Listed {
        match self {
            Self::Disk { root, .. } => folders
                .into_iter()
                .map(|rel| {
                    let listing = tree::list(root, &rel);
                    (rel, listing)
                })
                .collect(),
            Self::Server(workspace) => served::list(*workspace, folders),
            Self::There(there) => there.list(folders),
        }
    }

    pub(super) fn read(&self, rel: &Path) -> (Content, Option<Stamp>) {
        match self {
            Self::Disk { root, .. } => disk::read(&root.join(rel)),
            Self::Server(workspace) => served::read(*workspace, rel),
            Self::There(there) => there.read(rel),
        }
    }

    pub(super) fn stamp(&self, rel: &Path) -> Option<Stamp> {
        match self {
            Self::Disk { root, .. } => disk::stamp(&root.join(rel)),
            Self::Server(workspace) => served::stamp(*workspace, rel),
            Self::There(there) => there.stamp(rel),
        }
    }

    pub(super) fn search(&self, query: &str, mode: Mode) -> Option<Results> {
        match self {
            Self::Disk { .. } => None,
            Self::Server(workspace) => Some(served::search(*workspace, query, mode)),
            Self::There(there) => Some(there.search(query, mode)),
        }
    }
}
