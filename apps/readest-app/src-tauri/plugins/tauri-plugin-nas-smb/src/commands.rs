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

#[command]
pub(crate) async fn sync_read<R: Runtime>(
    app: AppHandle<R>,
    payload: SyncReadArgs,
) -> Result<SyncReadResponse> {
    app.nas_smb().sync_read(payload)
}

#[command]
pub(crate) async fn sync_write<R: Runtime>(
    app: AppHandle<R>,
    payload: SyncWriteArgs,
) -> Result<SyncSimpleResponse> {
    app.nas_smb().sync_write(payload)
}

#[command]
pub(crate) async fn sync_stat<R: Runtime>(
    app: AppHandle<R>,
    payload: SyncPathArgs,
) -> Result<SyncStatResponse> {
    app.nas_smb().sync_stat(payload)
}

#[command]
pub(crate) async fn sync_list<R: Runtime>(
    app: AppHandle<R>,
    payload: SyncPathArgs,
) -> Result<SyncListResponse> {
    app.nas_smb().sync_list(payload)
}

#[command]
pub(crate) async fn sync_mkdir<R: Runtime>(
    app: AppHandle<R>,
    payload: SyncPathArgs,
) -> Result<SyncSimpleResponse> {
    app.nas_smb().sync_mkdir(payload)
}

#[command]
pub(crate) async fn sync_remove_dir<R: Runtime>(
    app: AppHandle<R>,
    payload: SyncPathArgs,
) -> Result<SyncSimpleResponse> {
    app.nas_smb().sync_remove_dir(payload)
}
