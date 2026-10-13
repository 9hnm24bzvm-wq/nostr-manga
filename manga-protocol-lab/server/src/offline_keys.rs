//! Operator-run offline group-key verification and exclusive backup copying.
//! Secret bytes never appear in return values, command arguments or logs.
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
use zeroize::Zeroizing;

fn read_verified(path: &Path, expected: &str) -> Result<Zeroizing<Vec<u8>>> {
    ensure!(
        crate::auth::valid_hash(expected),
        "expected lowercase public key required"
    );
    let before = fs::symlink_metadata(path).context("cannot inspect source key")?;
    ensure!(
        before.file_type().is_file(),
        "source must be a regular non-symlink file"
    );
    let file = File::open(path).context("cannot open source key")?;
    let opened = file.metadata()?;
    ensure!(
        opened.is_file() && opened.len() <= 128,
        "source key file size/type rejected"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        ensure!(
            opened.dev() == before.dev() && opened.ino() == before.ino(),
            "source changed while opening"
        );
        ensure!(
            opened.permissions().mode() & 0o077 == 0,
            "source key permissions must exclude group and other users"
        );
    }
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(129).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 128, "source changed or exceeded size limit");
    let text = std::str::from_utf8(&bytes)
        .context("source key encoding rejected")?
        .trim();
    ensure!(
        crate::auth::valid_hash(text),
        "source must contain one lowercase secret key"
    );
    let raw = Zeroizing::new(hex::decode(text).context("source key encoding rejected")?);
    let secret = secp256k1::SecretKey::from_slice(&raw).context("source key scalar rejected")?;
    let pair = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &secret);
    ensure!(
        pair.x_only_public_key().0.to_string() == expected,
        "source does not match expected public key"
    );
    Ok(bytes)
}
pub fn verify(path: &Path, expected: &str) -> Result<()> {
    let _bytes = read_verified(path, expected)?;
    Ok(())
}
pub fn backup(source: &Path, destination: &Path, expected: &str) -> Result<()> {
    let bytes = read_verified(source, expected)?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory =
        File::open(parent).context("backup destination directory must already exist")?;
    ensure!(
        directory.metadata()?.is_dir(),
        "backup parent must be a directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            directory.metadata()?.permissions().mode() & 0o022 == 0,
            "backup directory must not be writable by group or other users"
        );
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut out = options
        .open(destination)
        .context("destination must be new; existing files are never overwritten")?;
    out.write_all(&bytes)
        .context("backup write failed; destination may be incomplete")?;
    out.sync_all()
        .context("backup sync failed; destination is unconfirmed")?;
    directory
        .sync_all()
        .context("backup directory sync failed; destination is unconfirmed")?;
    verify(destination, expected).context("backup readback verification failed")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> (tempfile::TempDir, std::path::PathBuf, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.key");
        fs::write(&path, format!("{:064x}", 1)).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let public = crate::auth::sign(&format!("{:064x}", 1), 1, vec![], String::new())
            .unwrap()
            .pubkey;
        (dir, path, public)
    }
    #[test]
    fn copies_verified_key_without_overwrite() {
        let (dir, path, public) = source();
        let dest = dir.path().join("backup.key");
        backup(&path, &dest, &public).unwrap();
        verify(&dest, &public).unwrap();
        assert_eq!(fs::read(&path).unwrap(), fs::read(&dest).unwrap());
        assert!(backup(&path, &dest, &public).is_err());
        let wrong = crate::auth::sign(&format!("{:064x}", 2), 1, vec![], String::new())
            .unwrap()
            .pubkey;
        assert!(backup(&path, &dir.path().join("wrong.key"), &wrong).is_err());
        assert!(!dir.path().join("wrong.key").exists());
    }
    #[test]
    fn rejects_invalid_and_oversized_keys() {
        let (_dir, path, public) = source();
        fs::write(&path, "0".repeat(129)).unwrap();
        assert!(verify(&path, &public).is_err());
        fs::write(&path, "0".repeat(64)).unwrap();
        assert!(verify(&path, &public).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_and_loose_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let (dir, path, public) = source();
        let link = dir.path().join("link");
        symlink(&path, &link).unwrap();
        assert!(verify(&link, &public).is_err());
        assert!(backup(&path, &link, &public).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(verify(&path, &public).is_err());
    }
}
