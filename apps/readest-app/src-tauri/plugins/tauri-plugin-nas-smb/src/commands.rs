use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::NasSmbExt;
use crate::Result;

#[command]
pub(crate) async fn connect<R: Runtime>(
    app: AppHandle<R>,
    payload: ConnectArgs,
) -> Result<ConnectResponse> {
    app.nas_smb().connect(payload)
}

#[command]
pub(crate) async fn list<R: Runtime>(app: AppHandle<R>, payload: ListArgs) -> Result<ListResponse> {
    app.nas_smb().list(payload)
}

#[command]
pub(crate) async fn download<R: Runtime>(
    app: AppHandle<R>,
    payload: DownloadArgs,
) -> Result<DownloadResponse> {
    app.nas_smb().download(payload)
}

#[command]
pub(crate) async fn disconnect<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.nas_smb().disconnect()
}

#[command]
pub(crate) async fn upload<R: Runtime>(
    app: AppHandle<R>,
    payload: UploadArgs,
) -> Result<UploadResponse> {
    app.nas_smb().upload(payload)
}

#[command]
pub(crate) async fn mkdir<R: Runtime>(
    app: AppHandle<R>,
    payload: MkdirArgs,
) -> Result<SimpleResponse> {
    app.nas_smb().mkdir(payload)
}

#[command]
pub(crate) async fn remove<R: Runtime>(
    app: AppHandle<R>,
    payload: RemoveArgs,
) -> Result<SimpleResponse> {
    app.nas_smb().remove(payload)
}

#[command]
pub(crate) async fn tailscale_status<R: Runtime>(app: AppHandle<R>) -> Result<TailscaleStatus> {
    app.nas_smb().tailscale_status()
}

#[command]
pub(crate) async fn open_tailscale<R: Runtime>(app: AppHandle<R>) -> Result<OpenTailscaleResponse> {
    app.nas_smb().open_tailscale()
}

#[command]
pub(crate) async fn uri_display_name<R: Runtime>(
    app: AppHandle<R>,
    payload: UriArgs,
) -> Result<UriNameResponse> {
    app.nas_smb().uri_display_name(payload)
}
