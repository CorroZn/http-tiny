// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static FILE_OPEN_FAILURES: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn report_open_failure(kind: io::ErrorKind) {
    let failures = FILE_OPEN_FAILURES.fetch_add(1, Ordering::Relaxed);
    if failures.trailing_zeros() >= 10 {
        crate::logging::log(format!("http-tiny: file open failed ({kind:?})\n"));
    }
}

pub(crate) struct DocumentRoot {
    canonical_path: PathBuf,
    /// Directory handle opened once at startup and reused across requests.
    root: std::fs::File,
}

impl DocumentRoot {
    #[cfg(unix)]
    pub(crate) fn open(path: &str) -> io::Result<Self> {
        let canonical_path = std::fs::canonicalize(path)?;
        let root = open_root_directory(&canonical_path)?;
        Ok(Self {
            canonical_path,
            root,
        })
    }

    #[cfg(not(unix))]
    pub(crate) fn open(_path: &str) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "document-root confinement is not supported on this platform",
        ))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.canonical_path
    }

    /// A freshly opened root handle when the configured path no longer refers to
    /// the pinned directory, or `None` when the pinned handle is still current.
    /// Re-walking from `/` on every request costs a dozen syscalls, so the
    /// handle is reused and only verified.
    #[cfg(unix)]
    fn refreshed_root(&self) -> io::Result<Option<std::fs::File>> {
        use rustix::fs::{AtFlags, CWD, fstat, statat};

        let pinned = fstat(&self.root).map_err(io::Error::from)?;
        let current =
            statat(CWD, &self.canonical_path, AtFlags::empty()).map_err(io::Error::from)?;
        if pinned.st_dev == current.st_dev && pinned.st_ino == current.st_ino {
            Ok(None)
        } else {
            Ok(Some(open_root_directory(&self.canonical_path)?))
        }
    }
}

// Choose flags that keep directory traversal pinned and prevent symlink following.
#[cfg(unix)]
fn directory_flags() -> rustix::fs::OFlags {
    use rustix::fs::OFlags;

    #[cfg(target_os = "linux")]
    {
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
    }
    #[cfg(not(target_os = "linux"))]
    {
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
    }
}

// Open the document root component-by-component so a swapped ancestor cannot redirect it.
#[cfg(unix)]
fn open_root_directory(path: &Path) -> io::Result<std::fs::File> {
    use rustix::fs::{Mode, open, openat};

    let mut directory = open("/", directory_flags(), Mode::empty()).map_err(io::Error::from)?;

    for component in path.components() {
        let name = match component {
            Component::RootDir => continue,
            Component::Normal(name) => name,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "document root contains an invalid component",
                ));
            }
        };
        directory =
            openat(&directory, name, directory_flags(), Mode::empty()).map_err(io::Error::from)?;
    }

    Ok(std::fs::File::from(directory))
}

// Open each requested path component relative to its pinned parent, rejecting symlinks.
#[cfg(unix)]
pub(crate) fn open_confined_file(docroot: &DocumentRoot, path: &Path) -> io::Result<std::fs::File> {
    use rustix::fs::{Mode, OFlags, openat};

    // Reuse the pinned root handle; it is re-opened only when the configured
    // path stopped referring to it.
    let refreshed = docroot.refreshed_root()?;
    let root_directory = refreshed.as_ref().unwrap_or(&docroot.root);
    let relative = path.strip_prefix(&docroot.canonical_path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "requested path is outside the document root",
        )
    })?;
    let component_count = relative.components().count();
    if component_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "requested path is not a file",
        ));
    }

    let mut directory = None;
    for (index, component) in relative.components().enumerate() {
        let Component::Normal(name) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "requested path contains an invalid component",
            ));
        };
        let is_last = index + 1 == component_count;

        let flags = if is_last {
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK
        } else {
            directory_flags()
        };
        let opened = match directory.as_ref() {
            Some(parent) => openat(parent, name, flags, Mode::empty()),
            None => openat(root_directory, name, flags, Mode::empty()),
        }
        .map_err(io::Error::from)?;
        let opened = std::fs::File::from(opened);
        if is_last {
            return Ok(opened);
        }
        directory = Some(opened);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "requested path has no file component",
    ))
}
