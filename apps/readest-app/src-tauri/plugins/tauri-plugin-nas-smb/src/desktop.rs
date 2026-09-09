use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

/// En escritorio no hay SMB (de momento): el plugin existe para que la app
/// compile igual en todas las plataformas, pero cada comando responde que no
/// está disponible en vez de fallar de forma rara.
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<NasSmb<R>> {
    Ok(NasSmb(app.clone()))
}

pub struct NasSmb<R: Runtime>(AppHandle<R>);

impl<R: Runtime> NasSmb<R> {
    pub fn connect(&self, _payload: ConnectArgs) -> crate::Result<ConnectResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn list(&self, _payload: ListArgs) -> crate::Result<ListResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn download(&self, _payload: DownloadArgs) -> crate::Result<DownloadResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn disconnect(&self) -> crate::Result<()> {
        Err(crate::Error::UnsupportedPlatformError)
    }
}
