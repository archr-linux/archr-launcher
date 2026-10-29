//! Bounded regular-file reads and same-directory atomic publication.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};

#[derive(Clone, Copy)]
pub enum Symlinks {
    Follow,
    Reject,
}

/// Open without waiting for a FIFO writer; validate the descriptor, not the path.
pub fn open_regular(path: &Path, symlinks: Symlinks) -> io::Result<File> {
    let flags = libc::O_NONBLOCK
        | match symlinks {
            Symlinks::Follow => 0,
            Symlinks::Reject => libc::O_NOFOLLOW,
        };
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid_data("expected a regular file"));
    }
    Ok(file)
}

/// Metadata is only an early rejection; the read itself always has a byte limit.
pub fn read_bounded(file: &File, limit: u64) -> io::Result<Vec<u8>> {
    if file.metadata()?.len() > limit {
        return Err(invalid_data("file exceeds size limit"));
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid_data("file exceeds size limit"));
    }
    Ok(bytes)
}

pub fn read_bytes(path: &Path, limit: u64, symlinks: Symlinks) -> io::Result<Vec<u8>> {
    read_bounded(&open_regular(path, symlinks)?, limit)
}

pub fn read_utf8(path: &Path, limit: u64, symlinks: Symlinks) -> io::Result<String> {
    String::from_utf8(read_bytes(path, limit, symlinks)?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Publish only a complete, synced temporary file. Durable data also syncs the
/// parent directory after rename; disposable caches and /run can skip that sync.
/// A directory-sync error is reported, but the rename has already happened.
pub fn atomic_publish(
    path: &Path,
    durable: bool,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let parent = parent_dir(path);
    create_dirs(parent, durable)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    write(temporary.as_file_mut())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    if durable {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

/// A secret (passwords, keys): mode 0600 before it becomes visible, durable.
pub fn publish_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_publish(path, true, |file| {
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.write_all(bytes)
    })
}

/// Remove a file (already missing is fine) and sync the removal.
pub fn remove_durable(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(()) => File::open(parent_dir(path))?.sync_all(),
    }
}

/// `create_dir_all`; durable data also syncs the parent of every folder it
/// had to create, so a power cut cannot lose the new folder's entry.
fn create_dirs(dir: &Path, durable: bool) -> io::Result<()> {
    let missing: Vec<&Path> = dir
        .ancestors()
        .take_while(|p| !p.as_os_str().is_empty() && !p.exists())
        .collect();
    fs::create_dir_all(dir)?;
    if durable {
        for created in missing.iter().rev() {
            File::open(parent_dir(created))?.sync_all()?;
        }
    }
    Ok(())
}

pub fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}
