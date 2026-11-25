//! Print to Rupert (#101): an entry outside Rupert that saves into the
//! inbox, so a PDF made from any app arrives like a scan.
//! - macOS: an alias to the inbox folder in `~/Library/PDF Services`. The
//!   Print dialog's PDF menu lists it ("Save to Rupert"); choosing it saves
//!   the PDF into the inbox.
//! - Windows: a shortcut to the inbox folder in File Explorer's Send to
//!   folder ("Rupert inbox").
//!
//! The entry is named in Rupert's language; [`install`] removes one made
//! earlier in any language first, and [`remove`] removes them all. Only
//! entries Rupert made (an alias or shortcut file under one of its names)
//! are ever removed, never a folder of the user's.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::i18n::{Lang, tr, tr_in};

/// The entry's name (without an extension), in every language.
fn english_name() -> &'static str {
    if cfg!(windows) {
        "Rupert inbox"
    } else {
        "Save to Rupert"
    }
}

/// The entry's name in Rupert's current language.
pub fn name() -> &'static str {
    if cfg!(windows) {
        tr("Rupert inbox")
    } else {
        tr("Save to Rupert")
    }
}

/// The file the entry is: an alias (no extension) or a shortcut (`.lnk`).
fn file_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.lnk")
    } else {
        name.to_string()
    }
}

/// Where the entry goes: `~/Library/PDF Services` (the real home folder,
/// also from inside the sandbox) or the Send to folder. `None` on Linux.
pub fn folder() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        real_home().map(|h| h.join("Library").join("PDF Services"))
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|a| {
            PathBuf::from(a)
                .join("Microsoft")
                .join("Windows")
                .join("SendTo")
        })
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        None
    }
}

/// The user's home folder as the system knows it (a sandboxed app's `HOME`
/// is its container).
#[cfg(target_os = "macos")]
fn real_home() -> Option<PathBuf> {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStrExt;
    // SAFETY: getpwuid returns a pointer into static storage, read at once.
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() || (*pw).pw_dir.is_null() {
            return None;
        }
        let dir = CStr::from_ptr((*pw).pw_dir);
        Some(PathBuf::from(std::ffi::OsStr::from_bytes(dir.to_bytes())))
    }
}

/// Puts the entry for `inbox` in `folder` (made if missing), named in
/// Rupert's language, replacing one made earlier. Returns its path.
pub fn install(folder: &Path, inbox: &Path) -> Result<PathBuf> {
    remove(folder)?;
    fs::create_dir_all(folder)?;
    let entry = folder.join(file_name(name()));
    if fs::symlink_metadata(&entry).is_ok() {
        // Something of the user's has that name: leave it alone.
        return Err(Error::InvalidValue(
            tr("Something with the entry's name is already there; rename or remove it first.")
                .into(),
        ));
    }
    write_link(&entry, inbox)?;
    Ok(entry)
}

/// Writes `entry` (an alias or shortcut) itself, for the Mac App Store
/// edition, where the user chose its place in a save panel.
pub fn write_at(entry: &Path, inbox: &Path) -> Result<()> {
    if fs::symlink_metadata(entry).is_ok() {
        fs::remove_file(entry)?;
    }
    write_link(entry, inbox)
}

/// Removes the entry from `folder`, in whichever language it was made.
pub fn remove(folder: &Path) -> Result<()> {
    for lang in Lang::ALL {
        let entry = folder.join(file_name(tr_in(lang, english_name())));
        // Only a file Rupert made: an alias or shortcut is a plain file; a
        // folder of the user's with that name is a directory.
        if fs::symlink_metadata(&entry).is_ok_and(|m| m.is_file()) {
            fs::remove_file(&entry)?;
        }
    }
    Ok(())
}

// MARK: - Links

#[cfg(target_os = "macos")]
fn write_link(entry: &Path, target: &Path) -> Result<()> {
    use objc2_foundation::{NSURL, NSURLBookmarkCreationOptions};
    let failed = |e: String| {
        Error::InvalidValue(format!(
            "{}: {e}",
            tr("couldn't make the Print dialog's entry")
        ))
    };
    let target_url = NSURL::from_path(target, true, None)
        .ok_or_else(|| Error::InvalidPath(target.display().to_string()))?;
    let entry_url = NSURL::from_path(entry, false, None)
        .ok_or_else(|| Error::InvalidPath(entry.display().to_string()))?;
    let data = target_url
        .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
            NSURLBookmarkCreationOptions::SuitableForBookmarkFile,
            None,
            None,
        )
        .map_err(|e| failed(e.localizedDescription().to_string()))?;
    NSURL::writeBookmarkData_toURL_options_error(&data, &entry_url, 0)
        .map_err(|e| failed(e.localizedDescription().to_string()))
}

#[cfg(windows)]
fn write_link(entry: &Path, target: &Path) -> Result<()> {
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        IPersistFile,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{HSTRING, Interface};
    let failed = |e: windows::core::Error| {
        Error::InvalidValue(format!("{}: {e}", tr("couldn't make the Send to entry")))
    };
    // SAFETY: plain COM calls on this thread; the strings outlive them.
    unsafe {
        // Already initialized on this thread is fine (S_FALSE / RPC_E_CHANGED_MODE).
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(failed)?;
        link.SetPath(&HSTRING::from(target)).map_err(failed)?;
        let file: IPersistFile = link.cast().map_err(failed)?;
        file.Save(&HSTRING::from(entry), true).map_err(failed)?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", windows)))]
fn write_link(_entry: &Path, _target: &Path) -> Result<()> {
    Err(Error::Unsupported(
        tr("there's no such entry on Linux").into(),
    ))
}

// MARK: - Tests

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn resolve_alias(entry: &Path) -> PathBuf {
        use objc2_foundation::{NSURL, NSURLBookmarkResolutionOptions};
        let url = NSURL::from_path(entry, false, None).unwrap();
        let data = NSURL::bookmarkDataWithContentsOfURL_error(&url).unwrap();
        let mut stale = objc2::runtime::Bool::NO;
        // SAFETY: `stale` outlives the call.
        let resolved = unsafe {
            NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
                &data,
                NSURLBookmarkResolutionOptions::WithoutUI,
                None,
                &mut stale,
            )
        }
        .unwrap();
        resolved.to_file_path().unwrap()
    }

    #[test]
    fn makes_replaces_and_removes_the_alias() {
        let dir = tempfile::tempdir().unwrap();
        let (services, inbox) = (dir.path().join("PDF Services"), dir.path().join("Inbox"));
        fs::create_dir(&inbox).unwrap();
        let entry = install(&services, &inbox).unwrap();
        assert_eq!(entry, services.join("Save to Rupert"));
        assert_eq!(
            resolve_alias(&entry).canonicalize().unwrap(),
            inbox.canonicalize().unwrap()
        );

        // A new inbox replaces it.
        let other = dir.path().join("New inbox");
        fs::create_dir(&other).unwrap();
        install(&services, &other).unwrap();
        assert_eq!(
            resolve_alias(&entry).canonicalize().unwrap(),
            other.canonicalize().unwrap()
        );

        // An entry made in another language goes too.
        let german = services.join(tr_in(Lang::De, "Save to Rupert"));
        fs::write(&german, b"alias").unwrap();
        remove(&services).unwrap();
        assert!(!entry.exists() && !german.exists());

        // A folder of the user's under that name stays, and install says so.
        fs::create_dir(services.join("Save to Rupert")).unwrap();
        remove(&services).unwrap();
        assert!(services.join("Save to Rupert").is_dir());
        assert!(install(&services, &inbox).is_err());
    }

    #[test]
    fn finds_the_pdf_services_folder_in_the_real_home() {
        let folder = folder().unwrap();
        assert!(folder.ends_with("Library/PDF Services"));
        assert!(!folder.to_string_lossy().contains("/Containers/"));
    }
}
