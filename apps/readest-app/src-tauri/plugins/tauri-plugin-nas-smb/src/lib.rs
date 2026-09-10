use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

// SMB está implementado en Android (Kotlin + smbj) y en Windows (std::fs sobre
// rutas UNC, que es como Windows expone un recurso compartido). macOS, Linux e
// iOS se quedan con la implementación de `desktop`, que responde "no
// disponible": así el plugin siempre arranca y no tumba la app.
#[cfg(all(any(desktop, target_os = "ios"), not(windows)))]
mod desktop;
#[cfg(target_os = "android")]
mod mobile;
// En Windows de verdad, y bajo `cargo test` en cualquier sistema: así la parte
// que no es FFI (rutas, errores, base64) se comprueba también desde Linux.
#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
mod windows_smb;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(all(any(desktop, target_os = "ios"), not(windows)))]
use desktop::NasSmb;
#[cfg(target_os = "android")]
use mobile::NasSmb;
#[cfg(windows)]
use windows_smb::NasSmb;

/// Acceso al plugin desde [`tauri::App`], [`tauri::AppHandle`] o [`tauri::Window`].
pub trait NasSmbExt<R: Runtime> {
    fn nas_smb(&self) -> &NasSmb<R>;
}

impl<R: Runtime, T: Manager<R>> crate::NasSmbExt<R> for T {
    fn nas_smb(&self) -> &NasSmb<R> {
        self.state::<NasSmb<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("nas-smb")
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::list,
            commands::download,
            commands::upload,
            commands::mkdir,
            commands::remove,
            commands::disconnect,
            commands::tailscale_status,
            commands::open_tailscale,
            commands::uri_display_name,
            commands::sync_read,
            commands::sync_write,
            commands::sync_stat,
            commands::sync_list,
            commands::sync_mkdir,
            commands::sync_remove_dir,
        ])
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let nas_smb = mobile::init(app, api)?;
            #[cfg(windows)]
            let nas_smb = windows_smb::init(app, api)?;
            #[cfg(all(any(desktop, target_os = "ios"), not(windows)))]
            let nas_smb = desktop::init(app, api)?;
            app.manage(nas_smb);
            Ok(())
        })
        .build()
}
