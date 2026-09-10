use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::*;

/// Engancha la clase Kotlin del plugin (NasSmbPlugin) al arrancar la app.
///
/// Este módulo se compila SOLO en Android (ver lib.rs). iOS y escritorio usan
/// `desktop.rs`, que responde "no disponible": si aquí devolviéramos error en
/// esas plataformas, el `setup` del plugin fallaría y no arrancaría ni la app.
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<NasSmb<R>> {
    let handle = api.register_android_plugin("com.readest.nas_smb", "NasSmbPlugin")?;
    Ok(NasSmb(handle))
}

pub struct NasSmb<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> NasSmb<R> {
    pub fn connect(&self, payload: ConnectArgs) -> crate::Result<ConnectResponse> {
        self.0
            .run_mobile_plugin("connect", payload)
            .map_err(Into::into)
    }

    pub fn list(&self, payload: ListArgs) -> crate::Result<ListResponse> {
        self.0.run_mobile_plugin("list", payload).map_err(Into::into)
    }

    pub fn download(&self, payload: DownloadArgs) -> crate::Result<DownloadResponse> {
        self.0
            .run_mobile_plugin("download", payload)
            .map_err(Into::into)
    }

    pub fn disconnect(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin("disconnect", ())
            .map_err(Into::into)
    }

    pub fn upload(&self, payload: UploadArgs) -> crate::Result<UploadResponse> {
        self.0
            .run_mobile_plugin("upload", payload)
            .map_err(Into::into)
    }

    pub fn mkdir(&self, payload: MkdirArgs) -> crate::Result<SimpleResponse> {
        self.0.run_mobile_plugin("mkdir", payload).map_err(Into::into)
    }

    pub fn remove(&self, payload: RemoveArgs) -> crate::Result<SimpleResponse> {
        self.0
            .run_mobile_plugin("remove", payload)
            .map_err(Into::into)
    }

    pub fn tailscale_status(&self) -> crate::Result<TailscaleStatus> {
        self.0
            .run_mobile_plugin("tailscale_status", ())
            .map_err(Into::into)
    }

    pub fn open_tailscale(&self) -> crate::Result<OpenTailscaleResponse> {
        self.0
            .run_mobile_plugin("open_tailscale", ())
            .map_err(Into::into)
    }
}
