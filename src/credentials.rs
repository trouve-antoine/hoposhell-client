use std::{
    fs::{self, DirBuilder, Permissions},
    io::{self, Write},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::Path,
};

pub fn prepare_home(path: &Path) -> io::Result<()> {
    prepare_directory(path, true)
}

fn prepare_directory(path: &Path, secure_existing: bool) -> io::Result<()> {
    let existed = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => true,
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Credentials directory must be a directory, not a symlink",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };

    DirBuilder::new().recursive(true).mode(0o700).create(path)?;
    if secure_existing || !existed {
        fs::set_permissions(path, Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    // Custom paths may live in shared directories; do not chmod existing parents.
    prepare_directory(parent, false)?;

    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Credentials destination must be a regular file, not a symlink",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    // A same-directory temporary file keeps secrets private from creation and
    // replaces old files atomically, without following destination symlinks.
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.as_file().set_permissions(Permissions::from_mode(0o600))?;
    file.write_all(contents)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn creates_private_directories_and_credentials() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("hoposhell");
        prepare_home(&home).unwrap();
        let key = home.join("nested/shell.pem");
        write(&key, b"private key").unwrap();

        assert_eq!(mode(&home), 0o700);
        assert_eq!(mode(key.parent().unwrap()), 0o700);
        assert_eq!(mode(&key), 0o600);
        assert_eq!(fs::read(&key).unwrap(), b"private key");
    }

    #[test]
    fn tightens_existing_home_permissions() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("hoposhell");
        fs::create_dir(&home).unwrap();
        fs::set_permissions(&home, Permissions::from_mode(0o755)).unwrap();

        prepare_home(&home).unwrap();

        assert_eq!(mode(&home), 0o700);
    }

    #[test]
    fn replaces_existing_credentials_with_private_permissions() {
        let root = tempfile::tempdir().unwrap();
        let key = root.path().join("shell.pem");
        fs::write(&key, b"old key").unwrap();
        fs::set_permissions(&key, Permissions::from_mode(0o644)).unwrap();

        write(&key, b"new key").unwrap();

        assert_eq!(mode(&key), 0o600);
        assert_eq!(fs::read(&key).unwrap(), b"new key");
    }

    #[test]
    fn preserves_existing_custom_parent_permissions() {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), Permissions::from_mode(0o755)).unwrap();

        write(&root.path().join("shell.pem"), b"private key").unwrap();

        assert_eq!(mode(root.path()), 0o755);
    }

    #[test]
    fn rejects_symlink_destinations_without_changing_the_target() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        fs::write(&target, b"original").unwrap();
        let key = root.path().join("shell.pem");
        symlink(&target, &key).unwrap();

        assert!(write(&key, b"private key").is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
    }

    #[test]
    fn rejects_symlink_directories() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("hoposhell");
        symlink(root.path(), &home).unwrap();

        assert!(prepare_home(&home).is_err());
        assert!(write(&home.join("shell.pem"), b"private key").is_err());
        assert!(!root.path().join("shell.pem").exists());
    }
}
