use std::fs;
use std::io::Write;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use crate::Result;
use crate::errors::AppError;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_write_with_privacy(path, bytes, false)
}

pub fn atomic_write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_write_with_privacy(path, bytes, true)
}

fn atomic_write_with_privacy(path: &Path, bytes: &[u8], private: bool) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::InvalidPath(format!("{} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;

    let temp_path = path.with_extension("tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if private {
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp_path)
        .map_err(|source| AppError::io(&temp_path, source))?;
    #[cfg(unix)]
    if private {
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|source| AppError::io(&temp_path, source))?;
    }
    #[cfg(not(unix))]
    let _ = private;

    file.write_all(bytes)
        .map_err(|source| AppError::io(&temp_path, source))?;
    file.sync_all()
        .map_err(|source| AppError::io(&temp_path, source))?;
    fs::rename(&temp_path, path).map_err(|source| AppError::io(path, source))?;

    Ok(())
}
