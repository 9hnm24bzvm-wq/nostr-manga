//! Native filesystem safety primitives. Unsupported operations fail closed.
use std::{fs::File, io, path::Path};

#[cfg(unix)]
pub fn file_identity(file: &File) -> io::Result<Vec<u8>> {
    use std::os::unix::fs::MetadataExt;
    let m = file.metadata()?;
    Ok([m.dev().to_be_bytes(), m.ino().to_be_bytes()].concat())
}
#[cfg(windows)]
pub fn file_identity(file: &File) -> io::Result<Vec<u8>> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx,
    };
    let mut info = std::mem::MaybeUninit::<FILE_ID_INFO>::uninit();
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            info.as_mut_ptr().cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    Ok([
        info.VolumeSerialNumber.to_be_bytes().as_slice(),
        info.FileId.Identifier.as_slice(),
    ]
    .concat())
}
pub fn same_file(a: &File, b: &File) -> io::Result<bool> {
    Ok(file_identity(a)? == file_identity(b)?)
}
#[cfg(unix)]
pub fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}
#[cfg(windows)]
fn volume_mount(path: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetVolumePathNameW;
    let name: Vec<u16> = path
        .canonicalize()?
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut mount = vec![0u16; 32768];
    if unsafe { GetVolumePathNameW(name.as_ptr(), mount.as_mut_ptr(), mount.len() as u32) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let end = mount
        .iter()
        .position(|c| *c == 0)
        .ok_or_else(|| io::Error::other("unterminated volume mount"))?;
    mount.truncate(end + 1);
    Ok(mount)
}
/// Windows has no POSIX directory fsync through ordinary File handles. Flush
/// the local volume instead, requiring an elevated service account. Failure is
/// never interpreted as a durable namespace mutation.
#[cfg(windows)]
pub fn sync_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::{ffi::OsStringExt, fs::OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW;
    let mount = volume_mount(path)?;
    let mut name = vec![0u16; 64];
    if unsafe {
        GetVolumeNameForVolumeMountPointW(mount.as_ptr(), name.as_mut_ptr(), name.len() as u32)
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let n = name
        .iter()
        .position(|c| *c == 0)
        .ok_or_else(|| io::Error::other("unterminated volume GUID"))?;
    if n == 0 || name[n - 1] != b'\\' as u16 {
        return Err(io::Error::other("invalid volume GUID"));
    }
    let name = std::ffi::OsString::from_wide(&name[..n - 1]);
    let volume = File::options()
        .write(true)
        .share_mode(3)
        .open(name)
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("durable Windows storage requires elevated volume-flush access: {e}"),
            )
        })?;
    volume.sync_all()
}
#[cfg(unix)]
pub fn available_space(owner: &File, _path: &Path) -> io::Result<Option<(u64, u64)>> {
    use std::os::fd::AsRawFd;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::fstatvfs(owner.as_raw_fd(), stats.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let s = unsafe { stats.assume_init() };
    Ok((s.f_blocks > 0).then_some((s.f_bavail as u64, s.f_blocks as u64)))
}
#[cfg(windows)]
pub fn available_space(_owner: &File, path: &Path) -> io::Result<Option<(u64, u64)>> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mount = volume_mount(path)?;
    let (mut free, mut total) = (0, 0);
    if unsafe { GetDiskFreeSpaceExW(mount.as_ptr(), &mut free, &mut total, std::ptr::null_mut()) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok((total > 0).then_some((free, total)))
}
/// Legacy nginx process ownership/reader-drain proofs exist only on Linux.
pub fn require_external_serving() -> anyhow::Result<()> {
    anyhow::ensure!(
        cfg!(target_os = "linux"),
        "managed external direct serving requires Linux; use native guarded HTTP originals"
    );
    Ok(())
}
/// Non-Linux does not claim an undocumented boot GUID. A stricter process
/// epoch prevents any old external hold from matching after process restart.
pub fn storage_epoch() -> io::Result<Vec<u8>> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read("/proc/sys/kernel/random/boot_id")
    }
    #[cfg(not(target_os = "linux"))]
    {
        static EPOCH: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
        let mut value = b"process-session-v1".to_vec();
        value.extend_from_slice(EPOCH.get_or_init(rand::random));
        Ok(value)
    }
}
