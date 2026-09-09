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
