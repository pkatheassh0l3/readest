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

    pub fn upload(&self, _payload: UploadArgs) -> crate::Result<UploadResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn mkdir(&self, _payload: MkdirArgs) -> crate::Result<SimpleResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn remove(&self, _payload: RemoveArgs) -> crate::Result<SimpleResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn tailscale_status(&self) -> crate::Result<TailscaleStatus> {
        Ok(TailscaleStatus { installed: false })
    }

    pub fn open_tailscale(&self) -> crate::Result<OpenTailscaleResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn uri_display_name(&self, _payload: UriArgs) -> crate::Result<UriNameResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn sync_read(&self, _payload: SyncReadArgs) -> crate::Result<SyncReadResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn sync_write(&self, _payload: SyncWriteArgs) -> crate::Result<SyncSimpleResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn sync_stat(&self, _payload: SyncPathArgs) -> crate::Result<SyncStatResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn sync_list(&self, _payload: SyncPathArgs) -> crate::Result<SyncListResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn sync_mkdir(&self, _payload: SyncPathArgs) -> crate::Result<SyncSimpleResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }

    pub fn sync_remove_dir(&self, _payload: SyncPathArgs) -> crate::Result<SyncSimpleResponse> {
        Err(crate::Error::UnsupportedPlatformError)
    }
}
