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
