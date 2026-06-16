use std::fs;
use std::io::Write;
use std::path::Path;

use crate::Result;
use crate::errors::AppError;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::InvalidPath(format!("{} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;

    let temp_path = path.with_extension("tmp");
    let mut file =
        fs::File::create(&temp_path).map_err(|source| AppError::io(&temp_path, source))?;
    file.write_all(bytes)
        .map_err(|source| AppError::io(&temp_path, source))?;
    file.sync_all()
        .map_err(|source| AppError::io(&temp_path, source))?;
    fs::rename(&temp_path, path).map_err(|source| AppError::io(path, source))?;

    Ok(())
}
