use std::{
    fs,
    io::{self, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
};

use crate::args::Args;

const IMGCAT_URL: &str = "https://raw.githubusercontent.com/gnachman/iterm2-shell-integration/db8a601d8037c6ce2296c4d64765869f13a5eb5a/utilities/imgcat";
const IMGCAT_CHECKSUM: &str = include_str!("../third-party/imgcat/CHECKSUM");

pub fn main_populate(args: Args) {
    if let Err(error) = populate(&args) {
        eprintln!("Unable to install imgcat: {error}");
        std::process::exit(1);
    }
}

fn populate(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let bin_folder_path = Path::new(&args.hoposhell_folder_path).join("bin");
    let imgcat_path = bin_folder_path.join("imgcat");
    eprintln!("Download pinned imgcat: {IMGCAT_URL}");
    let imgcat_contents = crate::buffer_limits::read(
        reqwest::blocking::get(IMGCAT_URL)?.error_for_status()?,
        "imgcat download",
    )?;
    install(&imgcat_path, &imgcat_contents, IMGCAT_CHECKSUM)?;
    eprintln!("Installed verified imgcat: {}", imgcat_path.display());
    Ok(())
}

fn verify_checksum(contents: &[u8], checksum: &str) -> io::Result<()> {
    let expected = checksum.split_whitespace().next().unwrap_or("");
    let actual: String = openssl::sha::sha256(contents)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("imgcat SHA-256 mismatch: expected {expected}, got {actual}"),
        ));
    }
    Ok(())
}

fn install(path: &Path, contents: &[u8], checksum: &str) -> io::Result<()> {
    // No filesystem changes until the full download has been verified.
    verify_checksum(contents, checksum)?;

    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Missing installation directory",
        )
    })?;
    match fs::symlink_metadata(parent) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Installation directory must not be a symlink",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir_all(parent)?,
        Err(error) => return Err(error),
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Installation destination must be a regular file, not a symlink",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(contents)?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(0o755))?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    const TEST_CHECKSUM: &str =
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  imgcat\n";

    #[test]
    fn verifies_exact_bytes() {
        assert!(verify_checksum(b"abc", TEST_CHECKSUM).is_ok());
        assert!(verify_checksum(b"abc\n", TEST_CHECKSUM).is_err());
        assert!(verify_checksum(b"modified", TEST_CHECKSUM).is_err());
        assert!(verify_checksum(b"abc", "").is_err());
    }

    #[test]
    fn checked_in_checksum_has_sha256sum_format() {
        let fields: Vec<_> = IMGCAT_CHECKSUM.split_whitespace().collect();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].len(), 64);
        assert!(fields[0].bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(fields[1], "imgcat");
    }

    #[test]
    fn installs_and_replaces_verified_executable() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("bin/imgcat");
        install(&path, b"abc", TEST_CHECKSUM).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"abc");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o755
        );

        fs::write(&path, b"old script").unwrap();
        install(&path, b"abc", TEST_CHECKSUM).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"abc");
    }

    #[test]
    fn mismatch_preserves_existing_installation() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("imgcat");
        fs::write(&path, b"old script").unwrap();

        assert!(install(&path, b"untrusted", TEST_CHECKSUM).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old script");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn mismatch_does_not_create_directories() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("bin/imgcat");
        assert!(install(&path, b"untrusted", TEST_CHECKSUM).is_err());
        assert!(!path.parent().unwrap().exists());
    }

    #[test]
    fn rejects_symlink_destinations() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        fs::write(&target, b"original").unwrap();
        let path = root.path().join("imgcat");
        symlink(&target, &path).unwrap();

        assert!(install(&path, b"abc", TEST_CHECKSUM).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
    }

    #[test]
    fn rejects_symlink_installation_directories() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("bin");
        symlink(root.path(), &directory).unwrap();

        assert!(install(&directory.join("imgcat"), b"abc", TEST_CHECKSUM).is_err());
        assert!(!root.path().join("imgcat").exists());
    }
}
