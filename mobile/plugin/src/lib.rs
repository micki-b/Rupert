//! Phone services for Rupert's phone app, on iOS (Swift in `ios/`) and
//! Android (Kotlin in `android/`): folder access through the Files picker
//! with bookmarks, fetching files from the sync service, previews, the
//! document camera, web sign-in and the keychain.
//!
//! Android has the camera (Google's ML Kit scanner), sign-in, the keychain,
//! previews and Google's own Drive authorization ([`Device::google_token`]);
//! the Files-picker calls are iOS-only for now.
//!
//! Every call blocks until the phone answers, so call from a background
//! thread. On a computer every call fails with [`Error::Unsupported`].

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_rupert_ios);

// MARK: - Types

#[derive(Debug)]
pub enum Error {
    Unsupported,
    Failed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => f.write_str("not available on this device"),
            Self::Failed(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// A folder the user picked, with a bookmark to reopen it after a relaunch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedFolder {
    pub path: String,
    pub name: String,
    /// Base64 bookmark data.
    pub bookmark: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedBookmark {
    pub path: Option<String>,
    /// A replacement for a stale bookmark; save it.
    pub bookmark: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placed {
    pub path: String,
    pub name: String,
}

#[derive(Deserialize)]
struct PickResult {
    folder: Option<PickedFolder>,
}

#[derive(Deserialize)]
struct ResolveResult {
    folders: Vec<ResolvedBookmark>,
}

#[derive(Deserialize)]
struct ScanResult {
    pages: Vec<String>,
}

#[derive(Serialize)]
struct PathArgs<'a> {
    path: &'a str,
}

// MARK: - Device

/// Handle to the phone side, managed as app state.
pub struct Device<R: Runtime> {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> Device<R> {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    fn call<T: serde::de::DeserializeOwned>(
        &self,
        command: &str,
        args: impl Serialize,
    ) -> Result<T> {
        self.handle
            .run_mobile_plugin(command, args)
            .map_err(|e| Error::Failed(clean(e.to_string())))
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    fn call<T: serde::de::DeserializeOwned>(
        &self,
        _command: &str,
        _args: impl Serialize,
    ) -> Result<T> {
        Err(Error::Unsupported)
    }

    /// Shows the Files folder picker. `None` if cancelled.
    pub fn pick_folder(&self) -> Result<Option<PickedFolder>> {
        Ok(self.call::<PickResult>("pickFolder", ())?.folder)
    }

    /// Reopens saved folders (in the same order) and keeps them open.
    pub fn resolve_bookmarks(&self, bookmarks: &[String]) -> Result<Vec<ResolvedBookmark>> {
        #[derive(Serialize)]
        struct Args<'a> {
            bookmarks: &'a [String],
        }
        Ok(self
            .call::<ResolveResult>("resolveBookmarks", Args { bookmarks })?
            .folders)
    }

    /// Has the sync service fetch every file in a hidden `.rupert` folder.
    pub fn fetch_folder(&self, path: &str) -> Result<()> {
        self.call::<serde_json::Value>("fetchFolder", PathArgs { path })
            .map(drop)
    }

    /// Downloads one document the user asked to open.
    pub fn download(&self, path: &str) -> Result<()> {
        self.call::<serde_json::Value>("download", PathArgs { path })
            .map(drop)
    }

    /// Shows a downloaded document in Quick Look.
    pub fn preview(&self, path: &str) -> Result<()> {
        self.call::<serde_json::Value>("preview", PathArgs { path })
            .map(drop)
    }

    /// Opens a web page in the phone's browser.
    pub fn open_url(&self, url: &str) -> Result<()> {
        #[derive(Serialize)]
        struct Args<'a> {
            url: &'a str,
        }
        self.call::<serde_json::Value>("openUrl", Args { url })
            .map(drop)
    }

    /// Runs Rupert's document camera. `mode` is "color", "gray" or "bw";
    /// `dpi` the output resolution. Page image files (JPEG, or PNG for
    /// black and white), empty if cancelled.
    pub fn scan(&self, mode: &str, dpi: u32) -> Result<Vec<String>> {
        #[derive(Serialize)]
        struct Args<'a> {
            mode: &'a str,
            dpi: u32,
        }
        Ok(self.call::<ScanResult>("scan", Args { mode, dpi })?.pages)
    }

    /// Copies `from` into `folder` as `name` (plus its extension, numbered
    /// if taken), coordinated with the sync service. Removes `from`'s
    /// temporary folder.
    pub fn place(&self, from: &str, folder: &str, name: &str) -> Result<Placed> {
        #[derive(Serialize)]
        struct Args<'a> {
            from: &'a str,
            folder: &'a str,
            name: &'a str,
        }
        self.call("place", Args { from, folder, name })
    }
}

impl<R: Runtime> Device<R> {
    /// Shows a web sign-in page; the redirect URL, or `None` if cancelled.
    pub fn sign_in(&self, url: &str, callback_scheme: &str) -> Result<Option<String>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            url: &'a str,
            callback_scheme: &'a str,
        }
        #[derive(Deserialize)]
        struct Answer {
            url: Option<String>,
        }
        Ok(self
            .call::<Answer>(
                "signIn",
                Args {
                    url,
                    callback_scheme,
                },
            )?
            .url)
    }

    /// Writes one of Rupert's small files (under a `.rupert` folder),
    /// coordinated with the sync service.
    pub fn write_file(&self, path: &str, text: &str) -> Result<()> {
        #[derive(Serialize)]
        struct Args<'a> {
            path: &'a str,
            text: &'a str,
        }
        self.call::<serde_json::Value>("writeFile", Args { path, text })
            .map(drop)
    }

    /// Removes one of Rupert's small files (and its `.rupert` folder if
    /// that is then empty).
    pub fn remove_file(&self, path: &str) -> Result<()> {
        self.call::<serde_json::Value>("removeFile", PathArgs { path })
            .map(drop)
    }

    /// Android: an access token for Google Drive with `scope`, from Google
    /// Play services (asks the person the first time; afterwards answers
    /// silently). `None` if they declined. Google allows the web sign-in
    /// used on iOS only for iOS apps.
    pub fn google_token(&self, scope: &str) -> Result<Option<String>> {
        #[derive(Serialize)]
        struct Args<'a> {
            scope: &'a str,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Token {
            access_token: Option<String>,
        }
        Ok(self
            .call::<Token>("googleToken", Args { scope })?
            .access_token)
    }

    pub fn keychain_get(&self, key: &str) -> Result<Option<String>> {
        #[derive(Serialize)]
        struct Args<'a> {
            key: &'a str,
        }
        #[derive(Deserialize)]
        struct Value {
            value: Option<String>,
        }
        Ok(self.call::<Value>("keychainGet", Args { key })?.value)
    }

    /// Stored for this device only, readable after the first unlock.
    pub fn keychain_set(&self, key: &str, value: &str) -> Result<()> {
        #[derive(Serialize)]
        struct Args<'a> {
            key: &'a str,
            value: &'a str,
        }
        self.call::<serde_json::Value>("keychainSet", Args { key, value })
            .map(drop)
    }

    pub fn keychain_delete(&self, key: &str) -> Result<()> {
        #[derive(Serialize)]
        struct Args<'a> {
            key: &'a str,
        }
        self.call::<serde_json::Value>("keychainDelete", Args { key })
            .map(drop)
    }

    /// The system's picture of a document on the phone (a JPEG fitting in
    /// `side` points, written to a temporary file whose path is returned),
    /// or `None` if there's none. Only for files that are on the phone: the
    /// caller checks, since asking about an online-only file could fetch it.
    pub fn thumbnail(&self, path: &str, side: u32) -> Result<Option<String>> {
        #[derive(Serialize)]
        struct Args<'a> {
            path: &'a str,
            side: u32,
        }
        #[derive(Deserialize)]
        struct Thumb {
            path: Option<String>,
        }
        Ok(self.call::<Thumb>("thumbnail", Args { path, side })?.path)
    }

    /// Where documents shared with Rupert from other apps wait to be sent to
    /// the inbox, one folder per document (`<id>/<file name>`), and the
    /// folder for the settings the iPhone's share extension reads.
    /// iOS: the app group shared with the extension; Android: the app's cache,
    /// where shares arriving by intent are copied.
    pub fn shared_folders(&self) -> Result<SharedFolders> {
        self.call::<SharedFolders>("sharedFolders", ())
    }
}

/// See [`Device::shared_folders`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedFolders {
    /// Documents waiting to be sent.
    pub pending: String,
    /// The share extension's settings file goes here (iOS only).
    pub settings: Option<String>,
}

/// Plugin errors arrive as JSON-ish text; keep just the message.
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
fn clean(message: String) -> String {
    serde_json::from_str::<serde_json::Value>(&message)
        .ok()
        .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or(message)
}

// MARK: - Plugin

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("rupert-phone")
        .setup(|app, _api| {
            #[cfg(target_os = "ios")]
            let device = Device {
                handle: _api.register_ios_plugin(init_plugin_rupert_ios)?,
            };
            #[cfg(target_os = "android")]
            let device = Device {
                handle: _api
                    .register_android_plugin("io.github.micki_b.rupert.phone", "RupertPlugin")?,
            };
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            let device = Device::<R> {
                _runtime: std::marker::PhantomData,
            };
            app.manage(device);
            Ok(())
        })
        .build()
}
