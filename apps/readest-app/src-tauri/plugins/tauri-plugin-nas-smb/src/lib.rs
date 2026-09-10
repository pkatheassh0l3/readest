use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

// SMB solo está implementado en Android (Kotlin + smbj). iOS y escritorio se
// quedan con la implementación de `desktop`, que responde "no disponible":
// así el plugin siempre arranca y no tumba la app en esas plataformas.
#[cfg(any(desktop, target_os = "ios"))]
mod desktop;
#[cfg(target_os = "android")]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(any(desktop, target_os = "ios"))]
use desktop::NasSmb;
#[cfg(target_os = "android")]
use mobile::NasSmb;

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
        ])
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let nas_smb = mobile::init(app, api)?;
            #[cfg(any(desktop, target_os = "ios"))]
            let nas_smb = desktop::init(app, api)?;
            app.manage(nas_smb);
            Ok(())
        })
        .build()
}
