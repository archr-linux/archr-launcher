use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

/// Check the path before opening (notably for FIFOs), then bound the actual read
/// as well as the metadata size in case the file grows while being read.
pub(crate) fn read_bounded(path: &Path, limit: u64) -> io::Result<String> {
    let check = |metadata: fs::Metadata| {
        if !metadata.is_file() || metadata.len() > limit {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected a regular file within the size limit",
            ))
        } else {
            Ok(())
        }
    };
    check(fs::metadata(path)?)?;
    let file = fs::File::open(path)?;
    check(file.metadata()?)?;
    let mut source = String::new();
    file.take(limit + 1).read_to_string(&mut source)?;
    if source.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file exceeds size limit",
        ));
    }
    Ok(source)
}

pub(crate) fn atomic_write(dir: &Path, name: &str, contents: &[u8]) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let mut temporary = tempfile::NamedTempFile::new_in(dir)?;
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(dir.join(name))
        .map_err(|error| error.error)?;
    Ok(())
}
