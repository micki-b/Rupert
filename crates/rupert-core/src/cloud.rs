//! Talking to the sync services through the file system: which service a
//! folder belongs to, downloading an online-only file, and freeing its space
//! again afterwards.
//!
//! Only used where the user asked for it (a source's "download online-only
//! files to index them" setting). Everywhere else Rupert never reads the
//! contents of online-only files.

use std::fs::File;
use std::io;
use std::path::Path;

use crate::error::Result;
use crate::source::Provider;

// MARK: - Provider

/// The sync service a folder belongs to, as the OS reports it (macOS File
/// Provider domains), checking the folder and then its parents. Catches
/// folders whose path doesn't give the service away, such as `~/Documents`
/// when iCloud's "Desktop & Documents Folders" is on.
#[cfg(target_os = "macos")]
pub fn provider_of(path: &Path) -> Option<Provider> {
    path.ancestors().find_map(|p| {
        let domain = xattr(p, "com.apple.file-provider-domain-id")?;
        Some(provider_for_domain(&domain))
    })
}

#[cfg(not(target_os = "macos"))]
pub fn provider_of(_path: &Path) -> Option<Provider> {
    None
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn provider_for_domain(domain: &str) -> Provider {
    let d = domain.to_ascii_lowercase();
    if d.contains("clouddocs") || d.contains("icloud") {
        Provider::ICloud
    } else if d.contains("onedrive") {
        Provider::OneDrive
    } else if d.contains("google") {
        Provider::GoogleDrive
    } else if d.contains("dropbox") {
        Provider::Dropbox
    } else {
        Provider::Local
    }
}

#[cfg(target_os = "macos")]
fn xattr(path: &Path, name: &str) -> Option<String> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let p = CString::new(path.as_os_str().as_bytes()).ok()?;
    let n = CString::new(name).ok()?;
    let mut buf = vec![0u8; 512];
    // SAFETY: valid C strings and a buffer of the stated length.
    let len = unsafe {
        libc::getxattr(
            p.as_ptr(),
            n.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if len <= 0 {
        return None;
    }
    buf.truncate(len as usize);
    String::from_utf8(buf).ok()
}

// MARK: - Download and evict

/// Downloads an online-only file by reading it through once (the OS
/// fetches it on first read). Blocks until it's local.
pub fn download(path: &Path) -> Result<()> {
    let mut f = File::open(path)?;
    io::copy(&mut f, &mut io::sink())?;
    Ok(())
}

/// Frees the local copy of a synced file, keeping it in the cloud
/// ("Remove Download" / "Free up space"). Returns `false` where the OS
/// offers no way to do this.
#[cfg(target_os = "macos")]
pub fn evict(path: &Path) -> Result<bool> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    NSFileManager::defaultManager()
        .evictUbiquitousItemAtURL_error(&url)
        .map_err(|e| {
            crate::error::Error::Io(io::Error::other(format!(
                "couldn't free the space: {}",
                e.localizedDescription()
            )))
        })?;
    Ok(true)
}

/// Windows: marking a placeholder "unpinned" tells OneDrive (and other
/// Cloud Files providers) to free its local copy.
#[cfg(windows)]
pub fn evict(path: &Path) -> Result<bool> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_PINNED, FILE_ATTRIBUTE_UNPINNED, GetFileAttributesW,
        INVALID_FILE_ATTRIBUTES, SetFileAttributesW,
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: NUL-terminated wide string.
    unsafe {
        let attrs = GetFileAttributesW(wide.as_ptr());
        if attrs == INVALID_FILE_ATTRIBUTES {
            return Err(io::Error::last_os_error().into());
        }
        let new = (attrs & !FILE_ATTRIBUTE_PINNED) | FILE_ATTRIBUTE_UNPINNED;
        if SetFileAttributesW(wide.as_ptr(), new) == 0 {
            return Err(io::Error::last_os_error().into());
        }
    }
    Ok(true)
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn evict(_path: &Path) -> Result<bool> {
    Ok(false)
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_file_provider_domains() {
        let cases = [
            (
                "com.apple.CloudDocs.iCloudDriveFileProvider/72658952-DD1B",
                Provider::ICloud,
            ),
            (
                "com.microsoft.OneDrive.FileProvider/OneDrive",
                Provider::OneDrive,
            ),
            ("com.google.drivefs.fpext/a@b.com", Provider::GoogleDrive),
            ("com.getdropbox.dropbox.fileprovider/x", Provider::Dropbox),
            ("com.example.other/x", Provider::Local),
        ];
        for (domain, expected) in cases {
            assert_eq!(provider_for_domain(domain), expected, "{domain}");
        }
    }

    #[test]
    fn plain_folders_have_no_provider() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(provider_of(dir.path()), None);
    }

    #[test]
    fn download_reads_local_files() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        std::fs::write(&f, "x").unwrap();
        download(&f).unwrap();
    }
}
