//! Share… and Email… (#69): hands a document to the system's sharing.
//! - macOS: the share menu (`NSSharingServicePicker`) at the pointer or
//!   button; Email… through Mail's compose service, the title as subject.
//! - Windows: the Share window (`DataTransferManager`); Email… through the
//!   default mail program (Simple MAPI), else the Share window.
//! - Linux: Email… through `xdg-email` (there's no common share window).
//!
//! What's sent is prepared by `rupert_core::share` (a renamed copy for an
//! inbox scan, a copy without notes). An online-only file is downloaded
//! first: the page hears `share-progress` while it is, and Cancel drops the
//! send once the download is done (a download can't be stopped halfway).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{AppState, CmdResult, err};

/// Sent while an online-only document is downloaded to send it (its name),
/// and with `null` when that's over.
const SHARE_PROGRESS: &str = "share-progress";

/// The latest send; Cancel moves it on, so an older one is dropped.
static LATEST: AtomicU64 = AtomicU64::new(0);

/// Where on the page the share menu opens: the pointer, or the button.
#[derive(Debug, Clone, Copy, Deserialize)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct At {
    x: f64,
    y: f64,
    #[serde(default)]
    width: f64,
    #[serde(default)]
    height: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloading: Option<String>,
}

/// The file(s) to send and the subject for an email. `None` when the send
/// was cancelled meanwhile.
async fn prepare(
    app: &AppHandle,
    source_id: &str,
    rel_path: &str,
    name: Option<String>,
) -> CmdResult<Option<(Vec<PathBuf>, String)>> {
    let state = app.state::<AppState>();
    let source = state.source(source_id)?;
    let include_notes = state.settings.lock().unwrap().get().send_notes;
    let title = state
        .index
        .lock()
        .unwrap()
        .doc_meta(source_id, rel_path)
        .ok()
        .flatten()
        .and_then(|m| m.title);
    let cache = app.path().app_cache_dir().map_err(err)?;
    let path = rupert_core::resolve(&source.root, rel_path).map_err(err)?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let subject = title
        .or_else(|| name.clone())
        .unwrap_or_else(|| rupert_core::rename::split_name(&file_name).0.to_string());

    let me = LATEST.fetch_add(1, Ordering::SeqCst) + 1;
    let online =
        rupert_core::cloud_state(&path).map_err(err)? == rupert_core::CloudState::CloudOnly;
    if online {
        let _ = app.emit(
            SHARE_PROGRESS,
            Progress {
                downloading: Some(file_name),
            },
        );
    }
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        rupert_core::share::prepare(&path, name.as_deref(), include_notes, &cache)
    })
    .await
    .map_err(err)?;
    if online {
        let _ = app.emit(SHARE_PROGRESS, Progress { downloading: None });
    }
    if LATEST.load(Ordering::SeqCst) != me {
        return Ok(None);
    }
    Ok(Some((vec![prepared.map_err(err)?], subject)))
}

/// Drops the send that's waiting for a download.
#[tauri::command]
pub fn share_cancel(app: AppHandle) {
    LATEST.fetch_add(1, Ordering::SeqCst);
    let _ = app.emit(SHARE_PROGRESS, Progress { downloading: None });
}

/// Share…: the system's share menu for the document `rel_path` of a source,
/// at `at` on the page. `name`: send it under this name (an inbox scan).
#[tauri::command]
pub async fn share_document(
    source_id: String,
    rel_path: String,
    name: Option<String>,
    at: At,
    window: WebviewWindow,
    app: AppHandle,
) -> CmdResult<()> {
    let Some((files, subject)) = prepare(&app, &source_id, &rel_path, name).await? else {
        return Ok(());
    };
    show_share(&window, files, subject, at)
}

/// Email…: a new message with the document attached and its title as the
/// subject.
#[tauri::command]
pub async fn email_document(
    source_id: String,
    rel_path: String,
    name: Option<String>,
    window: WebviewWindow,
    app: AppHandle,
) -> CmdResult<()> {
    let Some((files, subject)) = prepare(&app, &source_id, &rel_path, name).await? else {
        return Ok(());
    };
    compose_email(&window, files, subject).await
}

// MARK: - macOS

#[cfg(target_os = "macos")]
fn file_urls(files: &[PathBuf]) -> objc2::rc::Retained<objc2_foundation::NSArray> {
    use objc2::rc::Retained;
    use objc2_foundation::{NSArray, NSString, NSURL};
    let urls: Vec<Retained<objc2::runtime::AnyObject>> = files
        .iter()
        .map(|p| {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy()));
            Retained::into_super(Retained::into_super(url))
        })
        .collect();
    NSArray::from_retained_slice(&urls)
}

#[cfg(target_os = "macos")]
thread_local! {
    /// The share menu showing (kept alive while it's open).
    static PICKER: std::cell::RefCell<Option<objc2::rc::Retained<objc2_app_kit::NSSharingServicePicker>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_os = "macos")]
fn show_share(
    window: &WebviewWindow,
    files: Vec<PathBuf>,
    _subject: String,
    at: At,
) -> CmdResult<()> {
    let w = window.clone();
    window
        .run_on_main_thread(move || {
            use objc2::AnyThread;
            use objc2_app_kit::{NSSharingServicePicker, NSWindow};
            use objc2_foundation::{NSPoint, NSRect, NSRectEdge, NSSize};
            let Ok(ptr) = w.ns_window() else { return };
            // SAFETY: Tauri's NSWindow, alive as long as its window; main thread.
            let ns_window = unsafe { &*(ptr as *const NSWindow) };
            let Some(view) = ns_window.contentView() else {
                return;
            };
            let height = view.bounds().size.height;
            let (w_, h_) = (at.width.max(1.0), at.height.max(1.0));
            // The page's y runs down; the view's up, unless it's flipped.
            let y = if view.isFlipped() {
                at.y
            } else {
                height - at.y - h_
            };
            let rect = NSRect::new(NSPoint::new(at.x, y), NSSize::new(w_, h_));
            let items = file_urls(&files);
            // SAFETY: an array of file URLs, as the picker expects.
            let picker = unsafe {
                NSSharingServicePicker::initWithItems(NSSharingServicePicker::alloc(), &items)
            };
            picker.showRelativeToRect_ofView_preferredEdge(rect, &view, NSRectEdge::MinY);
            PICKER.with(|p| *p.borrow_mut() = Some(picker));
        })
        .map_err(err)
}

#[cfg(target_os = "macos")]
async fn compose_email(
    window: &WebviewWindow,
    files: Vec<PathBuf>,
    subject: String,
) -> CmdResult<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    window
        .run_on_main_thread(move || {
            use objc2_app_kit::{NSSharingService, NSSharingServiceNameComposeEmail};
            use objc2_foundation::NSString;
            // SAFETY: an AppKit constant.
            let name = unsafe { NSSharingServiceNameComposeEmail };
            let result = match NSSharingService::sharingServiceNamed(name) {
                Some(service) => {
                    service.setSubject(Some(&NSString::from_str(&subject)));
                    let items = file_urls(&files);
                    // SAFETY: an array of file URLs, as Mail expects.
                    unsafe {
                        if service.canPerformWithItems(Some(&items)) {
                            service.performWithItems(&items);
                            Ok(())
                        } else {
                            Err(())
                        }
                    }
                }
                None => Err(()),
            };
            let _ = tx.send(result);
        })
        .map_err(err)?;
    match rx.recv() {
        Ok(Ok(())) => Ok(()),
        _ => Err(
            rupert_core::i18n::tr("No mail app is set up to send it. Use Share… instead.").into(),
        ),
    }
}

// MARK: - Windows

#[cfg(target_os = "windows")]
fn show_share(
    window: &WebviewWindow,
    files: Vec<PathBuf>,
    subject: String,
    _at: At,
) -> CmdResult<()> {
    let hwnd = window.hwnd().map_err(err)?.0 as isize;
    // The files are looked up off the window's thread (it waits for them).
    let items = windows_share::storage_items(&files).map_err(err)?;
    let (tx, rx) = std::sync::mpsc::channel();
    window
        .run_on_main_thread(move || {
            let _ = tx.send(windows_share::show(hwnd, items, &subject).map_err(err));
        })
        .map_err(err)?;
    rx.recv().map_err(err)?
}

#[cfg(target_os = "windows")]
async fn compose_email(
    window: &WebviewWindow,
    files: Vec<PathBuf>,
    subject: String,
) -> CmdResult<()> {
    let (f, s) = (files.clone(), subject.clone());
    // The mail program's window is modal to this call: off the main thread.
    let sent = tauri::async_runtime::spawn_blocking(move || windows_share::mapi_send(&f, &s))
        .await
        .map_err(err)?;
    if sent {
        return Ok(());
    }
    // No mail program that speaks Simple MAPI (the new Outlook doesn't):
    // the Share window, which offers mail apps too.
    show_share(
        window,
        files,
        subject,
        At {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    )
}

#[cfg(target_os = "windows")]
mod windows_share {
    use std::cell::Cell;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;

    use windows::ApplicationModel::DataTransfer::{DataRequestedEventArgs, DataTransferManager};
    use windows::Foundation::TypedEventHandler;
    use windows::Storage::{IStorageItem, StorageFile};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::IDataTransferManagerInterop;
    use windows::core::{AgileReference, HSTRING, Interface, Ref, factory};
    use windows_collections::IIterable;

    thread_local! {
        /// The handler of the last share, removed before the next one.
        static TOKEN: Cell<Option<i64>> = const { Cell::new(None) };
    }

    /// The files as the Share window wants them (usable from its thread).
    pub fn storage_items(
        files: &[PathBuf],
    ) -> windows::core::Result<Vec<AgileReference<IStorageItem>>> {
        files
            .iter()
            .map(|p| {
                let item: IStorageItem =
                    StorageFile::GetFileFromPathAsync(&HSTRING::from(p.as_path()))?
                        .join()?
                        .cast()?;
                AgileReference::new(&item)
            })
            .collect()
    }

    /// Opens the Share window for `hwnd` with `items`.
    pub fn show(
        hwnd: isize,
        items: Vec<AgileReference<IStorageItem>>,
        title: &str,
    ) -> windows::core::Result<()> {
        let interop = factory::<DataTransferManager, IDataTransferManagerInterop>()?;
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        // SAFETY: Tauri's window handle, on its thread.
        let manager: DataTransferManager = unsafe { interop.GetForWindow(hwnd)? };
        if let Some(token) = TOKEN.take() {
            let _ = manager.RemoveDataRequested(token);
        }
        let title = HSTRING::from(title);
        let handler = TypedEventHandler::<DataTransferManager, DataRequestedEventArgs>::new(
            move |_, args: Ref<DataRequestedEventArgs>| {
                let data = args.ok()?.Request()?.Data()?;
                data.Properties()?.SetTitle(&title)?;
                let list: IIterable<IStorageItem> = items
                    .iter()
                    .map(|i| i.resolve().map(Some))
                    .collect::<windows::core::Result<Vec<_>>>()?
                    .into();
                data.SetStorageItemsReadOnly(&list)
            },
        );
        TOKEN.set(Some(manager.DataRequested(&handler)?));
        // SAFETY: as above.
        unsafe { interop.ShowShareUIForWindow(hwnd) }
    }

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Opens a new message in the default mail program (Simple MAPI) with
    /// the files attached. `false` if there's none that answers.
    pub fn mapi_send(files: &[PathBuf], subject: &str) -> bool {
        use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
        use windows_sys::Win32::System::Mapi::{
            MAPI_DIALOG, MAPI_LOGON_UI, MAPI_USER_ABORT, MapiFileDescW, MapiMessageW,
            SUCCESS_SUCCESS,
        };
        type SendMailW =
            unsafe extern "system" fn(usize, usize, *const MapiMessageW, u32, u32) -> u32;
        let dll = wide(std::ffi::OsStr::new("mapi32.dll"));
        // SAFETY: plain Win32 calls; the strings outlive the call.
        unsafe {
            let lib = LoadLibraryW(dll.as_ptr());
            if lib.is_null() {
                return false;
            }
            let Some(proc) = GetProcAddress(lib, c"MAPISendMailW".as_ptr().cast()) else {
                return false;
            };
            let send: SendMailW = std::mem::transmute(proc);
            let mut paths: Vec<Vec<u16>> = files.iter().map(|p| wide(p.as_os_str())).collect();
            let mut names: Vec<Vec<u16>> = files
                .iter()
                .map(|p| wide(p.file_name().unwrap_or_default()))
                .collect();
            let mut attachments: Vec<MapiFileDescW> = paths
                .iter_mut()
                .zip(names.iter_mut())
                .map(|(p, n)| MapiFileDescW {
                    nPosition: u32::MAX,
                    lpszPathName: p.as_mut_ptr(),
                    lpszFileName: n.as_mut_ptr(),
                    ..Default::default()
                })
                .collect();
            let mut subject = wide(std::ffi::OsStr::new(subject));
            let message = MapiMessageW {
                lpszSubject: subject.as_mut_ptr(),
                nFileCount: attachments.len() as u32,
                lpFiles: attachments.as_mut_ptr(),
                ..Default::default()
            };
            let result = send(0, 0, &message, MAPI_DIALOG | MAPI_LOGON_UI, 0);
            result == SUCCESS_SUCCESS || result == MAPI_USER_ABORT
        }
    }
}

// MARK: - Linux

#[cfg(target_os = "linux")]
fn show_share(
    _window: &WebviewWindow,
    _files: Vec<PathBuf>,
    _subject: String,
    _at: At,
) -> CmdResult<()> {
    Err(rupert_core::i18n::tr("Sharing isn't available on Linux; use Email….").into())
}

#[cfg(target_os = "linux")]
async fn compose_email(
    _window: &WebviewWindow,
    files: Vec<PathBuf>,
    subject: String,
) -> CmdResult<()> {
    let mut cmd = std::process::Command::new("xdg-email");
    cmd.arg("--subject").arg(&subject);
    for f in &files {
        cmd.arg("--attach").arg(f);
    }
    cmd.spawn().map(|_| ()).map_err(|_| {
        rupert_core::i18n::tr("Rupert can't open your mail program: xdg-email isn't installed.")
            .into()
    })
}
