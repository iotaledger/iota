// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Replace a secret file atomically, using an owner-only temporary file on Unix.
/// The parent directory must be trusted. A destination symlink is kept and its
/// target is replaced; a dangling one is an error. Existing permissive
/// permissions are not copied to the new file.
pub(crate) fn write_secret_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_secret_file_with(path, |file| file.write_all(contents))
}

fn write_secret_file_with(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let path = &resolve_symlink(path)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    #[cfg(unix)]
    let directory = File::open(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        temporary
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    write(temporary.as_file_mut())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    // The rename must also reach disk. If this fails the replacement is already
    // complete, but its durability is not guaranteed, so propagate the error.
    #[cfg(unix)]
    directory.sync_all()?;
    Ok(())
}

fn resolve_symlink(path: &Path) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::canonicalize(path),
        Ok(_) => Ok(path.to_path_buf()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn failed_write_preserves_previous_secret_and_removes_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("secret");
        fs::write(&path, b"previous secret").unwrap();
        let result = write_secret_file_with(&path, |file| {
            file.write_all(b"partial replacement")?;
            // A failed disk write must not truncate the old file.
            Err(io::Error::other("injected write failure"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"previous secret");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_rename_preserves_destination_and_removes_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("destination-directory");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("existing"), b"previous secret").unwrap();
        assert!(write_secret_file(&path, b"replacement").is_err());
        assert_eq!(fs::read(path.join("existing")).unwrap(), b"previous secret");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn temporary_secret_is_private_before_writing() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("secret");
        write_secret_file_with(&path, |file| {
            assert_eq!(file.metadata()?.permissions().mode() & 0o777, 0o600);
            file.write_all(b"secret")
        })
        .unwrap();
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn destination_symlink_is_kept_and_its_target_replaced() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target");
        let path = directory.path().join("secret");
        fs::write(&target, b"previous secret").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        symlink(&target, &path).unwrap();
        write_secret_file(&path, b"secret").unwrap();
        assert!(
            fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&target).unwrap(), b"secret");
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_destination_symlink_is_an_error() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("secret");
        symlink(directory.path().join("missing"), &path).unwrap();
        assert!(write_secret_file(&path, b"secret").is_err());
        assert!(
            fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn replacement_does_not_truncate_an_open_previous_file() {
        use std::io::Read;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("secret");
        fs::write(&path, b"previous secret").unwrap();
        let mut previous = File::open(&path).unwrap();
        write_secret_file(&path, b"replacement").unwrap();
        let mut contents = Vec::new();
        previous.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"previous secret");
        assert_eq!(fs::read(path).unwrap(), b"replacement");
    }
}
