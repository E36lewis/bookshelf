//! Windows only: makes a file private to the current user, as `chmod 600`
//! does elsewhere. Built and tested only on Windows (CI), so it's kept small.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, HANDLE};
use windows_sys::Win32::Security::Authorization::{
    SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, NO_MULTIPLE_TRUSTEE, SET_ACCESS,
    SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
};
use windows_sys::Win32::Security::{
    CreateWellKnownSid, GetTokenInformation, TokenUser, WinLocalSystemSid, ACL,
    DACL_SECURITY_INFORMATION, NO_INHERITANCE, PROTECTED_DACL_SECURITY_INFORMATION, PSID,
    SECURITY_MAX_SID_SIZE, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Replaces the file's access list with full access for the current user
/// and SYSTEM, nobody else. The list is marked protected, so nothing is
/// inherited from the folder (say a shared or synced one).
pub fn restrict_to_owner(path: &Path) -> io::Result<()> {
    let wide = wide_path(path)?;
    let user = current_user()?;
    // A SID's fields need 4-byte alignment, which u32s give.
    let mut system = [0u32; SECURITY_MAX_SID_SIZE as usize / 4];
    let mut size = SECURITY_MAX_SID_SIZE;
    // SAFETY: `system` is SECURITY_MAX_SID_SIZE bytes, the size passed.
    let made = unsafe {
        CreateWellKnownSid(
            WinLocalSystemSid,
            null_mut(),
            system.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if made == 0 {
        return Err(io::Error::last_os_error());
    }
    let entries = [
        full_access(user.sid()),
        full_access(system.as_mut_ptr().cast()),
    ];
    let mut acl: *mut ACL = null_mut();
    // SAFETY: the SIDs the entries point at outlive the call; the new
    // list is freed below.
    let err = unsafe { SetEntriesInAclW(entries.len() as u32, entries.as_ptr(), null(), &mut acl) };
    if err != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(err as i32));
    }
    // SAFETY: `wide` ends in a NUL; `acl` is the list made above.
    let err = unsafe {
        SetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl,
            null(),
        )
    };
    // SAFETY: SetEntriesInAclW allocates with LocalAlloc; freed once.
    unsafe { LocalFree(acl.cast()) };
    if err != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(err as i32));
    }
    Ok(())
}

fn full_access(sid: PSID) -> EXPLICIT_ACCESS_W {
    EXPLICIT_ACCESS_W {
        grfAccessPermissions: FILE_ALL_ACCESS,
        grfAccessMode: SET_ACCESS,
        grfInheritance: NO_INHERITANCE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid.cast(),
        },
    }
}

fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains a NUL",
        ));
    }
    wide.push(0);
    Ok(wide)
}

/// This process's TOKEN_USER, which holds the current user's SID. u64s
/// give the buffer the alignment of the pointer inside it.
struct CurrentUser(Vec<u64>);

impl CurrentUser {
    fn sid(&self) -> PSID {
        // SAFETY: filled in by GetTokenInformation(TokenUser) below; the
        // SID it points at lives in this same buffer.
        unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }
}

fn current_user() -> io::Result<CurrentUser> {
    let mut token: HANDLE = null_mut();
    // SAFETY: GetCurrentProcess is a pseudo-handle that needs no closing.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let result = token_user(token);
    // SAFETY: opened above, closed once.
    unsafe { CloseHandle(token) };
    result
}

fn token_user(token: HANDLE) -> io::Result<CurrentUser> {
    let mut len = 0u32;
    // Asks for the size: fails with ERROR_INSUFFICIENT_BUFFER by design.
    // SAFETY: a null buffer of length 0 is allowed.
    unsafe { GetTokenInformation(token, TokenUser, null_mut(), 0, &mut len) };
    if len == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `len` bytes.
    let ok =
        unsafe { GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), len, &mut len) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(CurrentUser(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Security::Authorization::GetNamedSecurityInfoW;
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorControl, PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED,
    };

    #[test]
    fn only_the_owner_and_system_are_let_in() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.sqlite3");
        std::fs::write(&path, "x").unwrap();
        restrict_to_owner(&path).unwrap();
        // Still ours to write, read and (when the folder goes) delete.
        std::fs::write(&path, "y").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "y");

        let wide = wide_path(&path).unwrap();
        let mut dacl: *mut ACL = null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        // SAFETY: out-pointers to locals; `sd` is freed below.
        let err = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut dacl,
                null_mut(),
                &mut sd,
            )
        };
        assert_eq!(err, ERROR_SUCCESS);
        let (mut control, mut revision) = (0u16, 0u32);
        // SAFETY: `sd` and `dacl` came from GetNamedSecurityInfoW.
        let (ok, aces) = unsafe {
            let ok = GetSecurityDescriptorControl(sd, &mut control, &mut revision);
            let aces = (*dacl).AceCount;
            LocalFree(sd);
            (ok, aces)
        };
        assert_ne!(ok, 0);
        assert_ne!(control & SE_DACL_PROTECTED, 0, "nothing inherited");
        assert_eq!(aces, 2, "the user and SYSTEM");
    }
}
