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
