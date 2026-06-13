use std::fs;
use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;

use crate::Result;
use crate::errors::AppError;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::InvalidPath(format!("{} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;

    let mut file = NamedTempFile::new_in(parent).map_err(|source| AppError::io(parent, source))?;
    file.write_all(bytes)
        .map_err(|source| AppError::io(file.path(), source))?;
    file.as_file_mut()
        .sync_all()
        .map_err(|source| AppError::io(file.path(), source))?;
    file.persist(path)
        .map_err(|error| AppError::io(path, error.error))?;

    Ok(())
}
