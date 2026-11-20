//! Talking to the sync services through the file system: which service a
//! folder belongs to, whether a drive streams its files on demand without
//! saying which are local, downloading an online-only file, and freeing its
//! space again afterwards.
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

// MARK: - Streaming drives

/// Whether `path` is on a drive that fetches files from the cloud when they
/// are read, without marking which ones are online-only: Google Drive for
/// desktop on Windows in "Stream files" mode (its own `G:` drive). OneDrive
/// and iCloud Drive mark online-only files, and Google Drive does on macOS,
/// so this is only ever true on Windows. Everything on such a drive counts
/// as online-only: reading it would download it.
#[cfg(windows)]
pub fn streams_on_demand(path: &Path) -> bool {
    use std::collections::HashMap;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};

    static KNOWN: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut root = [0u16; 261];
    // SAFETY: NUL-terminated input; the buffer's length is passed.
    if unsafe { GetVolumePathNameW(wide.as_ptr(), root.as_mut_ptr(), root.len() as u32) } == 0 {
        return false;
    }
    let root_len = root.iter().position(|&c| c == 0).unwrap_or(root.len());
    let key = PathBuf::from(String::from_utf16_lossy(&root[..root_len]));
    let known = KNOWN.get_or_init(Default::default);
    if let Some(&streams) = known.lock().unwrap().get(&key) {
        return streams;
    }
    let mut name = [0u16; 261];
    let mut fs_name = [0u16; 261];
    let (mut serial, mut max_len, mut flags) = (0u32, 0u32, 0u32);
    // SAFETY: `root` is NUL-terminated; buffers and their lengths match.
    let ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            name.as_mut_ptr(),
            name.len() as u32,
            &mut serial,
            &mut max_len,
            &mut flags,
            fs_name.as_mut_ptr(),
            fs_name.len() as u32,
        )
    } != 0;
    let text = |buf: &[u16]| {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    };
    let streams = ok && is_streaming_volume(&text(&name), &text(&fs_name), flags);
    known.lock().unwrap().insert(key, streams);
    streams
}

#[cfg(not(windows))]
pub fn streams_on_demand(_path: &Path) -> bool {
    false
}

/// `FILE_SUPPORTS_REMOTE_STORAGE`: the volume keeps files elsewhere.
const FILE_SUPPORTS_REMOTE_STORAGE: u32 = 0x0000_0100;

/// Google Drive for desktop's streaming drive, from its volume information:
/// named "Google Drive", or (should the name be translated) the FAT32 file
/// system it presents claiming remote storage, which a real FAT32 disk never
/// does.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_streaming_volume(name: &str, fs_name: &str, flags: u32) -> bool {
    name.trim().eq_ignore_ascii_case("Google Drive")
        || (fs_name.eq_ignore_ascii_case("FAT32") && flags & FILE_SUPPORTS_REMOTE_STORAGE != 0)
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
    fn recognizes_google_drives_streaming_drive() {
        // As Windows reports G: with Google Drive for desktop streaming.
        assert!(is_streaming_volume(
            "Google Drive",
            "FAT32",
            FILE_SUPPORTS_REMOTE_STORAGE
        ));
        assert!(is_streaming_volume("google drive", "FAT32", 0));
        // A translated name: still FAT32 claiming remote storage.
        assert!(is_streaming_volume("Google Drive (G:)x", "FAT32", 0x106));
        // Ordinary disks and USB sticks.
        assert!(!is_streaming_volume("OS", "NTFS", 0x3e7_00ff));
        assert!(!is_streaming_volume("USB STICK", "FAT32", 0x0000_0006));
        assert!(!is_streaming_volume("Backup", "exFAT", 0));
    }

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
