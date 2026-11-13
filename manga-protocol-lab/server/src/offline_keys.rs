//! Operator-run offline group-key verification and exclusive backup copying.
//! Secret bytes never appear in return values, command arguments or logs.
use anyhow::{Context, Result, ensure};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use zeroize::Zeroizing;

fn read_verified(path: &Path, expected: &str) -> Result<Zeroizing<Vec<u8>>> {
    ensure!(
        crate::auth::valid_hash(expected),
        "expected lowercase public key required"
    );
    let file =
        crate::private_files::open_private(path).context("cannot open private source key")?;
    ensure!(file.metadata()?.len() <= 128, "source key size rejected");
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let directory = File::open(parent).context("backup parent must exist")?;
        ensure!(
            directory.metadata()?.is_dir(),
            "backup parent must be a directory"
        );
        ensure!(
            directory.metadata()?.permissions().mode() & 0o022 == 0,
            "backup directory must not be writable by group or other users"
        );
    }
    #[cfg(windows)]
    crate::private_files::verify_private_directory(parent)?;
    let mut out = crate::private_files::create_private(destination)
        .context("destination must be new; existing files are never overwritten")?;
    out.write_all(&bytes)
        .context("backup write failed; destination may be incomplete")?;
    out.sync_all()
        .context("backup sync failed; destination is unconfirmed")?;
    crate::platform_fs::sync_directory(parent)
        .context("backup directory sync failed; destination is unconfirmed")?;
    verify(destination, expected).context("backup readback verification failed")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn source() -> (tempfile::TempDir, std::path::PathBuf, String) {
        let dir = tempfile::tempdir().unwrap();
        let private = dir.path().join("private");
        crate::private_files::create_private_directory(&private).unwrap();
        let path = private.join("source.key");
        let mut file = crate::private_files::create_private(&path).unwrap();
        write!(file, "{:064x}", 1).unwrap();
        file.sync_all().unwrap();
        drop(file);
        let public = crate::auth::sign(&format!("{:064x}", 1), 1, vec![], String::new())
            .unwrap()
            .pubkey;
        (dir, path, public)
    }
    #[test]
    fn copies_verified_key_without_overwrite() {
        let (_dir, path, public) = source();
        let dest = path.parent().unwrap().join("backup.key");
        backup(&path, &dest, &public).unwrap();
        verify(&dest, &public).unwrap();
        assert_eq!(fs::read(&path).unwrap(), fs::read(&dest).unwrap());
        assert!(backup(&path, &dest, &public).is_err());
        let wrong = crate::auth::sign(&format!("{:064x}", 2), 1, vec![], String::new())
            .unwrap()
            .pubkey;
        assert!(backup(&path, &path.parent().unwrap().join("wrong.key"), &wrong).is_err());
        assert!(!path.parent().unwrap().join("wrong.key").exists());
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
