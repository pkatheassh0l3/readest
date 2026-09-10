use serde::{Deserialize, Serialize};

/// Los datos de conexión al NAS. `port` y `domain` son opcionales: por defecto
/// 445 y dominio vacío, que es lo que espera Samba/TrueNAS.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectArgs {
    pub host: String,
    pub port: Option<u16>,
    pub share: String,
    pub username: String,
    pub password: String,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResponse {
    pub ok: bool,
    /// Mensaje de error en cristiano cuando `ok` es false.
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListArgs {
    /// Ruta dentro del recurso compartido; vacía es la raíz.
    pub path: String,
}

/// Los tamaños y fechas viajan como f64 porque el puente con Kotlin es JSON y
/// en JavaScript no hay enteros de 64 bits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmbEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size: f64,
    pub modified: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResponse {
    pub entries: Vec<SmbEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadArgs {
    /// Ruta del archivo dentro del recurso compartido.
    pub path: String,
    /// Ruta local absoluta donde dejarlo.
    pub dest: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResponse {
    pub ok: bool,
    pub bytes: f64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadArgs {
    /// Archivo local que se sube.
    pub source: String,
    /// Ruta de destino dentro del recurso compartido.
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadResponse {
    pub ok: bool,
    pub bytes: f64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MkdirArgs {
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveArgs {
    pub path: String,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleResponse {
    pub ok: bool,
    pub message: Option<String>,
}

/// Estado de la app de Tailscale en el dispositivo. `installed` es false también
/// cuando no se puede consultar (en Android 11+ hace falta declararla en
/// <queries> del manifiesto para poder verla; el plugin ya lo hace).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TailscaleStatus {
    pub installed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenTailscaleResponse {
    pub opened: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UriArgs {
    pub uri: String,
}

/// Nombre real ("display name") de un content:// del selector de Android.
/// Sin esto, el nombre habría que sacarlo de la propia URI, que en Android es
/// opaca (algo como `msf%3A1000000033`): los archivos acabarían en el NAS con
/// un nombre ilegible y sin extensión.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UriNameResponse {
    pub name: String,
}

// ---------------------------------------------------------------------------
// Comandos de sincronización
//
// Readest ya trae un motor de sincronización por archivos con proveedores
// intercambiables (WebDAV, S3, Drive...). Para enchufarle el NAS hace falta un
// puñado de operaciones que el explorador no necesitaba: leer y escribir
// archivos pequeños en memoria, y saber si algo existe.
//
// A diferencia de los comandos del explorador, estos NO rechazan la promesa:
// devuelven un sobre con un `code` normalizado, porque el motor de Readest
// distingue entre "no existe", "credenciales mal" y "no llego" para decidir si
// aborta la pasada o sigue. Un mensaje de error en prosa no da para eso.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPathArgs {
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReadArgs {
    pub path: String,
    /// Con `true` el contenido vuelve en base64; con `false`, como texto UTF-8.
    pub binary: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncWriteArgs {
    pub path: String,
    /// Texto UTF-8, o base64 si `binary` es true.
    pub content: String,
    pub binary: bool,
}

/// `code` es uno de AUTH_FAILED, NOT_FOUND, NETWORK, CONFLICT o UNKNOWN, los
/// mismos que usa `FileSyncError` en el frontend.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReadResponse {
    pub ok: bool,
    pub code: Option<String>,
    pub message: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatResponse {
    pub ok: bool,
    pub code: Option<String>,
    pub message: Option<String>,
    pub size: f64,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncListResponse {
    pub ok: bool,
    pub code: Option<String>,
    pub message: Option<String>,
    pub entries: Vec<SmbEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSimpleResponse {
    pub ok: bool,
    pub code: Option<String>,
    pub message: Option<String>,
}
