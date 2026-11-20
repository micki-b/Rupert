fn main() {
    // The app's Microsoft registration (a public client id, not a secret),
    // from `onedrive-client-id` or the environment. Without it the app
    // builds and runs, with OneDrive turned off.
    println!("cargo:rerun-if-changed=onedrive-client-id");
    println!("cargo:rerun-if-env-changed=RUPERT_ONEDRIVE_CLIENT_ID");
    if std::env::var_os("RUPERT_ONEDRIVE_CLIENT_ID").is_none()
        && let Ok(id) = std::fs::read_to_string("onedrive-client-id")
    {
        println!("cargo:rustc-env=RUPERT_ONEDRIVE_CLIENT_ID={}", id.trim());
    }
    // The same for Google (an iOS client id), from `google-client-id`.
    println!("cargo:rerun-if-changed=google-client-id");
    println!("cargo:rerun-if-env-changed=RUPERT_GOOGLE_CLIENT_ID");
    if std::env::var_os("RUPERT_GOOGLE_CLIENT_ID").is_none()
        && let Ok(id) = std::fs::read_to_string("google-client-id")
    {
        println!("cargo:rustc-env=RUPERT_GOOGLE_CLIENT_ID={}", id.trim());
    }
    tauri_build::build()
}
