use manga_node::platform_fs;
use std::{
    fs::{self, File},
    io::Write,
};

#[test]
fn hard_links_share_identity_but_copies_do_not() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let original = root.path().join("original");
    fs::write(&original, b"accepted")?;
    fs::hard_link(&original, root.path().join("alias"))?;
    fs::copy(&original, root.path().join("copy"))?;
    assert!(platform_fs::same_file(
        &File::open(&original)?,
        &File::open(root.path().join("alias"))?
    )?);
    assert!(!platform_fs::same_file(
        &File::open(&original)?,
        &File::open(root.path().join("copy"))?
    )?);
    Ok(())
}
#[test]
fn namespace_mutations_pass_durability_barrier() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let a = root.path().join("staged");
    let b = root.path().join("accepted");
    let mut file = File::create(&a)?;
    file.write_all(b"accepted")?;
    file.sync_all()?;
    drop(file);
    fs::rename(&a, &b)?;
    platform_fs::sync_directory(root.path())?;
    assert_eq!(fs::read(&b)?, b"accepted");
    fs::remove_file(&b)?;
    platform_fs::sync_directory(root.path())?;
    Ok(())
}
#[test]
fn capacity_is_bounded() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let f = File::create(root.path().join("owner"))?;
    if let Some((free, total)) = platform_fs::available_space(&f, root.path())? {
        assert!(total > 0 && free <= total);
    }
    Ok(())
}
#[test]
fn storage_epoch_is_stable_in_process() -> anyhow::Result<()> {
    assert_eq!(platform_fs::storage_epoch()?, platform_fs::storage_epoch()?);
    Ok(())
}
#[cfg(not(target_os = "linux"))]
#[test]
fn external_serving_is_rejected() {
    assert!(platform_fs::require_external_serving().is_err());
}
