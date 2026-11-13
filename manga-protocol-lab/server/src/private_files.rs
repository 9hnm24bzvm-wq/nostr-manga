//! Private local files. Windows ACLs admit only the current account, SYSTEM,
//! and Administrators; unsupported ACLs and reparse points fail closed.
use anyhow::{Result, ensure};
use std::{fs::File, path::Path};

pub fn open_private(path: &Path) -> Result<File> {
    let before = std::fs::symlink_metadata(path)?;
    ensure!(
        before.is_file() && !before.file_type().is_symlink(),
        "private file must be regular and not a symlink"
    );
    let mut options = File::options();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(path)?;
    ensure!(file.metadata()?.is_file(), "private file must be regular");
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            file.metadata()?.file_attributes() & 0x400 == 0,
            "private file reparse points are forbidden"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened = file.metadata()?;
        ensure!(
            opened.dev() == before.dev() && opened.ino() == before.ino(),
            "private file changed while opening"
        );
    }
    let named = File::open(path)?;
    ensure!(
        crate::platform_fs::same_file(&file, &named)?,
        "private file changed while opening"
    );
    verify_private(&file)?;
    Ok(file)
}

pub fn verify_private(file: &File) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            file.metadata()?.permissions().mode() & 0o077 == 0,
            "private file must not be group/world accessible"
        );
    }
    #[cfg(windows)]
    windows::verify(file)?;
    Ok(())
}

pub fn create_private(path: &Path) -> Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        return Ok(File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?);
    }
    #[cfg(windows)]
    {
        windows::create(path)
    }
}

pub fn create_private_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(path)?;
        Ok(())
    }
    #[cfg(windows)]
    {
        windows::create_directory(path)
    }
}

pub fn verify_private_directory(path: &Path) -> Result<()> {
    let mut options = File::options();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x02000000 | 0x00200000); // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
    }
    let file = options.open(path)?;
    ensure!(file.metadata()?.is_dir(), "private directory required");
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            file.metadata()?.file_attributes() & 0x400 == 0,
            "private directory reparse points are forbidden"
        );
    }
    verify_private(&file)
}

#[cfg(windows)]
mod windows {
    use super::*;
    use anyhow::Context;
    use std::{
        ffi::c_void,
        os::windows::{
            ffi::OsStrExt,
            io::{AsRawHandle, FromRawHandle},
        },
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE, LocalFree},
        Security::Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            GetSecurityInfo, SE_FILE_OBJECT,
        },
        Security::{
            ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION, GetTokenInformation,
            OWNER_SECURITY_INFORMATION, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
        },
        Storage::FileSystem::{CREATE_NEW, CreateDirectoryW, CreateFileW, FILE_ATTRIBUTE_NORMAL},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    struct Local(*mut c_void);
    impl Drop for Local {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    fn sid_string(sid: *mut c_void) -> Result<String> {
        let mut text = ptr::null_mut();
        ensure!(
            unsafe { ConvertSidToStringSidW(sid, &mut text) } != 0,
            "cannot decode ACL SID: {}",
            std::io::Error::last_os_error()
        );
        let _owned = Local(text.cast());
        let mut length = 0;
        while unsafe { *text.add(length) } != 0 {
            length += 1;
        }
        Ok(String::from_utf16(unsafe {
            std::slice::from_raw_parts(text, length)
        })?)
    }
    fn current_sid() -> Result<String> {
        let mut token = ptr::null_mut();
        ensure!(
            unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } != 0,
            "cannot inspect account token"
        );
        struct Token(*mut c_void);
        impl Drop for Token {
            fn drop(&mut self) {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
        let _token = Token(token);
        let mut needed = 0;
        unsafe {
            GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut needed);
        }
        ensure!(needed > 0, "cannot size account token");
        let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
        ensure!(
            unsafe {
                GetTokenInformation(
                    token,
                    TokenUser,
                    buffer.as_mut_ptr().cast(),
                    needed,
                    &mut needed,
                )
            } != 0,
            "cannot read account token"
        );
        let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        sid_string(user.User.Sid)
    }
    pub(super) fn verify(file: &File) -> Result<()> {
        let current = current_sid()?;
        let mut owner = ptr::null_mut();
        let mut dacl: *mut ACL = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        let status = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                ptr::null_mut(),
                &mut dacl,
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        ensure!(status == 0, "cannot inspect private ACL: {status}");
        let _descriptor = Local(descriptor);
        ensure!(
            !owner.is_null() && !dacl.is_null(),
            "private file requires owner and DACL"
        );
        let trusted = |sid: &str| sid == current || sid == "S-1-5-18" || sid == "S-1-5-32-544";
        ensure!(
            trusted(&sid_string(owner)?),
            "private file owner is not trusted"
        );
        for index in 0..unsafe { (*dacl).AceCount } {
            let mut ace = ptr::null_mut();
            ensure!(
                unsafe { windows_sys::Win32::Security::GetAce(dacl, index as u32, &mut ace) } != 0,
                "cannot inspect ACL entry"
            );
            let header = unsafe { &*ace.cast::<ACE_HEADER>() };
            if header.AceType == 1 {
                continue;
            } // deny ACE grants nothing
            ensure!(
                header.AceType == 0
                    && header.AceSize as usize >= std::mem::size_of::<ACCESS_ALLOWED_ACE>(),
                "unsupported private ACL entry"
            );
            let allowed = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
            let sid = sid_string((&allowed.SidStart as *const u32).cast_mut().cast())?;
            ensure!(
                trusted(&sid),
                "private ACL grants access to another account"
            );
        }
        Ok(())
    }
    fn descriptor(directory: bool) -> Result<Local> {
        let sid = current_sid()?;
        let inherit = if directory { "OICI" } else { "" };
        let sddl: Vec<u16> =
            format!("D:P(A;{inherit};FA;;;SY)(A;{inherit};FA;;;BA)(A;{inherit};FA;;;{sid})")
                .encode_utf16()
                .chain(Some(0))
                .collect();
        let mut result = ptr::null_mut();
        ensure!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    1,
                    &mut result,
                    ptr::null_mut(),
                )
            } != 0,
            "cannot create private ACL"
        );
        Ok(Local(result))
    }
    pub(super) fn create(path: &Path) -> Result<File> {
        let descriptor = descriptor(false)?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                0x80000000 | 0x40000000,
                7,
                &attributes,
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        };
        ensure!(
            handle != INVALID_HANDLE_VALUE,
            "cannot exclusively create private file: {}",
            std::io::Error::last_os_error()
        );
        let file = unsafe { File::from_raw_handle(handle) };
        verify(&file).context("new private ACL verification")?;
        Ok(file)
    }
    pub(super) fn create_directory(path: &Path) -> Result<()> {
        let descriptor = descriptor(true)?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        ensure!(
            unsafe { CreateDirectoryW(name.as_ptr(), &attributes) } != 0,
            "cannot exclusively create private directory: {}",
            std::io::Error::last_os_error()
        );
        super::verify_private_directory(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn private_creation_is_exclusive_and_readable() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("key");
        let mut file = create_private(&path).unwrap();
        file.write_all(b"fixture").unwrap();
        file.sync_all().unwrap();
        drop(file);
        let mut data = String::new();
        open_private(&path)
            .unwrap()
            .read_to_string(&mut data)
            .unwrap();
        assert_eq!(data, "fixture");
        assert!(create_private(&path).is_err());
    }
    #[test]
    fn private_directory_creation_is_exclusive() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("private");
        create_private_directory(&path).unwrap();
        verify_private_directory(&path).unwrap();
        assert!(create_private_directory(&path).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn rejects_public_permissions_and_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("key");
        drop(create_private(&path).unwrap());
        let link = root.path().join("link");
        symlink(&path, &link).unwrap();
        assert!(open_private(&link).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(open_private(&path).is_err());
    }
}
