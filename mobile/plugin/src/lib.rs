//! iOS services for Rupert's phone app (Swift in `ios/`): folder access
//! through the Files picker with bookmarks, fetching files from the sync
//! service, Quick Look and the document camera.
//!
//! Every call blocks until iOS answers, so call from a background thread.
//! Elsewhere every call fails with [`Error::Unsupported`].

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
            Self::Unsupported => f.write_str("only available on iPhone and iPad"),
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

/// Handle to the iOS side, managed as app state.
pub struct Device<R: Runtime> {
    #[cfg(target_os = "ios")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "ios"))]
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> Device<R> {
    #[cfg(target_os = "ios")]
    fn call<T: serde::de::DeserializeOwned>(
        &self,
        command: &str,
        args: impl Serialize,
    ) -> Result<T> {
        self.handle
            .run_mobile_plugin(command, args)
            .map_err(|e| Error::Failed(clean(e.to_string())))
    }

    #[cfg(not(target_os = "ios"))]
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

    /// Runs the document camera; JPEG page files, empty if cancelled.
    pub fn scan(&self) -> Result<Vec<String>> {
        Ok(self.call::<ScanResult>("scan", ())?.pages)
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

/// Plugin errors arrive as JSON-ish text; keep just the message.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn clean(message: String) -> String {
    serde_json::from_str::<serde_json::Value>(&message)
        .ok()
        .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or(message)
}

// MARK: - Plugin

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("rupert-ios")
        .setup(|app, _api| {
            #[cfg(target_os = "ios")]
            let device = Device {
                handle: _api.register_ios_plugin(init_plugin_rupert_ios)?,
            };
            #[cfg(not(target_os = "ios"))]
            let device = Device::<R> {
                _runtime: std::marker::PhantomData,
            };
            app.manage(device);
            Ok(())
        })
        .build()
}
