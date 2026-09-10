//! Acceso SMB en Windows.
//!
//! Aquí no hace falta ninguna biblioteca SMB (en Android la hace smbj): Windows
//! habla SMB de fábrica y un recurso compartido es, literalmente, una ruta —
//! `\\host\share\carpeta\libro.epub`. Así que todo el trabajo lo hace
//! `std::fs`, y lo único que hay que pedirle al sistema es que abra la sesión
//! con las credenciales del NAS, para lo que están `WNetAddConnection2W` y
//! `WNetCancelConnection2W` de mpr.dll.
//!
//! El módulo se compila en Windows y, además, bajo `cargo test` en cualquier
//! sistema: así la parte que no es FFI —resolución de rutas y traducción de
//! errores, que es donde de verdad se cuelan los fallos— se puede comprobar
//! desde Linux, donde se desarrolla esto.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

/// SMB en Windows va siempre por el 445; no hay forma de indicar otro puerto a
/// través de WNet. Mejor decirlo que fingir que se respeta.
const SMB_PORT: u16 = 445;

/// Tope para las lecturas en memoria de la sincronización, igual que en Android.
const MAX_SYNC_READ_BYTES: u64 = 16 * 1024 * 1024;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<NasSmb<R>> {
    Ok(NasSmb {
        _app: app.clone(),
        session: Mutex::new(None),
    })
}

/// La conexión viva: la raíz UNC del recurso compartido.
struct Session {
    unc_root: String,
}

pub struct NasSmb<R: Runtime> {
    _app: AppHandle<R>,
    session: Mutex<Option<Session>>,
}

// ---------------------------------------------------------------------------
// Rutas
// ---------------------------------------------------------------------------

/// Une la raíz UNC con una ruta relativa del recurso compartido.
///
/// Devuelve `None` si la ruta intenta salirse del share (`..`) o trae una
/// unidad o raíz propia: son rutas que el frontend no genera nunca, así que si
/// aparecen es que algo va mal y es mejor no tocar el disco.
fn resolve_path(unc_root: &str, path: &str) -> Option<PathBuf> {
    let mut out = String::from(unc_root.trim_end_matches(['/', '\\']));
    for segment in path.split(['/', '\\']) {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." || segment.contains(':') {
            return None;
        }
        out.push('\\');
        out.push_str(segment);
    }
    Some(PathBuf::from(out))
}

/// Ruta relativa al share, con barras normales, tal como la espera el frontend.
fn child_path(base: &str, name: &str) -> String {
    let base = base.trim_matches(['/', '\\']);
    if base.is_empty() {
        name.to_string()
    } else {
        format!("{base}/{name}")
    }
}

// ---------------------------------------------------------------------------
// Errores
// ---------------------------------------------------------------------------

/// Traduce el error de E/S al vocabulario de `FileSyncError` del frontend.
fn error_code(e: &io::Error) -> &'static str {
    match e.kind() {
        io::ErrorKind::NotFound => "NOT_FOUND",
        io::ErrorKind::PermissionDenied => "AUTH_FAILED",
        io::ErrorKind::AlreadyExists => "CONFLICT",
        _ => "NETWORK",
    }
}

/// Mensaje en cristiano para un error de E/S contra el share.
fn describe_io(e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::NotFound => "No se encuentra esa carpeta o archivo en el NAS.".to_string(),
        io::ErrorKind::PermissionDenied => {
            "Acceso denegado. Revisa en TrueNAS, en Sharing → SMB, que este usuario esté \
             permitido en la ACL del recurso compartido."
                .to_string()
        }
        _ => format!("Error hablando con el NAS: {e}"),
    }
}

/// Traduce el código que devuelve WNet a algo que diga QUÉ hay que tocar.
///
/// Windows devuelve el error como número, lo que es bastante mejor que la
/// versión de Android: allí hay que mirar el texto de la excepción de smbj.
fn describe_wnet_error(code: u32) -> String {
    match code {
        5 => "Acceso denegado a este recurso compartido. Revisa en TrueNAS, en Sharing → SMB, \
              que este usuario esté permitido en la ACL del share."
            .to_string(),
        53 | 1203 => "No se encuentra el NAS en la red. ¿Está encendido y, si estás fuera de \
                      casa, activo Tailscale?"
            .to_string(),
        67 => "El recurso compartido no existe con ese nombre. Comprueba el nombre exacto en \
               TrueNAS → Sharing → Windows Shares (SMB)."
            .to_string(),
        86 | 1326 => "Usuario o contraseña incorrectos. En TrueNAS: edita el usuario, activa \
                      \"Samba Authentication\" y vuelve a guardar la contraseña (aunque ya \
                      estuviera activada, hazlo de nuevo para regenerar el hash SMB)."
            .to_string(),
        1219 => "Windows ya tiene una sesión abierta con este NAS y otro usuario. Cierra esa \
                 conexión (en el Explorador, o con `net use \\\\host\\share /delete`) y vuelve \
                 a intentarlo."
            .to_string(),
        1231 => "La red no llega hasta el NAS. ¿Está activo Tailscale?".to_string(),
        1244 => "Windows no aceptó las credenciales para este recurso compartido.".to_string(),
        other => format!("Windows no pudo conectar con el recurso compartido (error {other})."),
    }
}

// ---------------------------------------------------------------------------
// Sesión de red
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod wnet {
    //! Enlace mínimo y hecho a mano con mpr.dll.
    //!
    //! Son dos funciones: no merece la pena arrastrar `windows-sys` entero (y
    //! su versión, que habría que mantener a la par del resto del árbol) por
    //! ellas.
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    const RESOURCE_GLOBALNET: u32 = 0x0000_0002;
    const RESOURCETYPE_DISK: u32 = 0x0000_0001;
    /// La conexión no se guarda en el perfil del usuario: muere con la sesión.
    const CONNECT_TEMPORARY: u32 = 0x0000_0004;

    const ERROR_SUCCESS: u32 = 0;
    const ERROR_ALREADY_ASSIGNED: u32 = 85;
    const ERROR_SESSION_CREDENTIAL_CONFLICT: u32 = 1219;

    #[repr(C)]
    struct NetResourceW {
        dw_scope: u32,
        dw_type: u32,
        dw_display_type: u32,
        dw_usage: u32,
        lp_local_name: *mut u16,
        lp_remote_name: *mut u16,
        lp_comment: *mut u16,
        lp_provider: *mut u16,
    }

    #[link(name = "mpr")]
    unsafe extern "system" {
        fn WNetAddConnection2W(
            net_resource: *mut NetResourceW,
            password: *const u16,
            username: *const u16,
            flags: u32,
        ) -> u32;
        fn WNetCancelConnection2W(name: *const u16, flags: u32, force: i32) -> u32;
    }

    fn wide(value: &str) -> Vec<u16> {
        std::ffi::OsStr::new(value)
            .encode_wide()
            .chain(once(0))
            .collect()
    }

    fn add_connection(unc_root: &str, username: &str, password: &str) -> u32 {
        let mut remote = wide(unc_root);
        let user = wide(username);
        let pass = wide(password);
        let mut resource = NetResourceW {
            dw_scope: RESOURCE_GLOBALNET,
            dw_type: RESOURCETYPE_DISK,
            dw_display_type: 0,
            dw_usage: 0,
            lp_local_name: std::ptr::null_mut(),
            lp_remote_name: remote.as_mut_ptr(),
            lp_comment: std::ptr::null_mut(),
            lp_provider: std::ptr::null_mut(),
        };
        // SAFETY: los tres punteros apuntan a buffers vivos durante la llamada
        // y terminados en NUL; el resto de campos van a null, como pide la API
        // para una conexión sin letra de unidad.
        unsafe {
            WNetAddConnection2W(
                &mut resource,
                pass.as_ptr(),
                user.as_ptr(),
                CONNECT_TEMPORARY,
            )
        }
    }

    fn cancel_connection(unc_root: &str) -> u32 {
        let remote = wide(unc_root);
        // SAFETY: `remote` sigue vivo durante la llamada y acaba en NUL.
        unsafe { WNetCancelConnection2W(remote.as_ptr(), 0, 1) }
    }

    /// Abre la sesión con el recurso compartido. `Ok(())` o el código de Windows.
    pub fn connect(unc_root: &str, username: &str, password: &str) -> Result<(), u32> {
        match add_connection(unc_root, username, password) {
            ERROR_SUCCESS | ERROR_ALREADY_ASSIGNED => Ok(()),
            // Ya hay una sesión con este servidor y otras credenciales: Windows
            // no permite dos a la vez, así que se cierra la anterior y se
            // reintenta una sola vez.
            ERROR_SESSION_CREDENTIAL_CONFLICT => {
                cancel_connection(unc_root);
                match add_connection(unc_root, username, password) {
                    ERROR_SUCCESS | ERROR_ALREADY_ASSIGNED => Ok(()),
                    code => Err(code),
                }
            }
            code => Err(code),
        }
    }

    pub fn disconnect(unc_root: &str) {
        cancel_connection(unc_root);
    }
}

#[cfg(not(windows))]
mod wnet {
    pub fn connect(_unc_root: &str, _username: &str, _password: &str) -> Result<(), u32> {
        Err(u32::MAX)
    }
    pub fn disconnect(_unc_root: &str) {}
}

// ---------------------------------------------------------------------------
// Tailscale
// ---------------------------------------------------------------------------

/// Ruta del cliente de Tailscale en Windows, si está instalado.
fn tailscale_exe() -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(dir) = std::env::var_os(var) {
            roots.push(PathBuf::from(dir));
        }
    }
    for root in roots {
        let exe = root.join("Tailscale").join("tailscale-ipn.exe");
        if exe.is_file() {
            return Some(exe);
        }
    }
    None
}

impl<R: Runtime> NasSmb<R> {
    /// Raíz UNC de la conexión viva, o el error que espera cada comando.
    fn unc_root(&self) -> Option<String> {
        self.session
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|s| s.unc_root.clone()))
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, SyncSimpleResponse> {
        let root = self.unc_root().ok_or_else(no_connection)?;
        resolve_path(&root, path).ok_or_else(|| SyncSimpleResponse {
            ok: false,
            code: Some("UNKNOWN".into()),
            message: Some("Ruta no válida".into()),
        })
    }

    pub fn connect(&self, payload: ConnectArgs) -> crate::Result<ConnectResponse> {
        if payload.port.unwrap_or(SMB_PORT) != SMB_PORT {
            return Ok(ConnectResponse {
                ok: false,
                message: Some(format!(
                    "En Windows el SMB va siempre por el puerto {SMB_PORT}; no se puede usar otro."
                )),
            });
        }
        let unc_root = format!(r"\\{}\{}", payload.host.trim(), payload.share.trim());
        let username = match payload.domain.as_deref().map(str::trim) {
            Some(domain) if !domain.is_empty() => format!("{domain}\\{}", payload.username),
            _ => payload.username.clone(),
        };

        if let Err(code) = wnet::connect(&unc_root, &username, &payload.password) {
            return Ok(ConnectResponse {
                ok: false,
                message: Some(describe_wnet_error(code)),
            });
        }
        // La sesión puede abrirse y el share seguir sin ser legible (ACL): se
        // comprueba leyendo la raíz antes de dar la conexión por buena.
        if let Err(e) = fs::read_dir(&unc_root) {
            wnet::disconnect(&unc_root);
            return Ok(ConnectResponse {
                ok: false,
                message: Some(describe_io(&e)),
            });
        }
        if let Ok(mut guard) = self.session.lock() {
            *guard = Some(Session { unc_root });
        }
        Ok(ConnectResponse {
            ok: true,
            message: None,
        })
    }

    pub fn disconnect(&self) -> crate::Result<()> {
        if let Ok(mut guard) = self.session.lock() {
            if let Some(session) = guard.take() {
                wnet::disconnect(&session.unc_root);
            }
        }
        Ok(())
    }

    pub fn list(&self, payload: ListArgs) -> crate::Result<ListResponse> {
        let dir = self
            .resolve(&payload.path)
            .map_err(|e| crate::Error::SmbError(e.message.unwrap_or_default()))?;
        let mut entries = read_entries(&dir, &payload.path)
            .map_err(|e| crate::Error::SmbError(describe_io(&e)))?;
        // Carpetas primero y por orden alfabético, como cualquier explorador.
        entries.sort_by(|a, b| {
            b.is_directory
                .cmp(&a.is_directory)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(ListResponse { entries })
    }

    pub fn download(&self, payload: DownloadArgs) -> crate::Result<DownloadResponse> {
        let src = match self.resolve(&payload.path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(DownloadResponse {
                    ok: false,
                    bytes: 0.0,
                    message: e.message,
                })
            }
        };
        let dest = PathBuf::from(&payload.dest);
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match fs::copy(&src, &dest) {
            Ok(bytes) => Ok(DownloadResponse {
                ok: true,
                bytes: bytes as f64,
                message: None,
            }),
            Err(e) => {
                // No dejamos medio archivo suelto.
                let _ = fs::remove_file(&dest);
                Ok(DownloadResponse {
                    ok: false,
                    bytes: 0.0,
                    message: Some(describe_io(&e)),
                })
            }
        }
    }

    pub fn upload(&self, payload: UploadArgs) -> crate::Result<UploadResponse> {
        let dst = match self.resolve(&payload.path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(UploadResponse {
                    ok: false,
                    bytes: 0.0,
                    message: e.message,
                })
            }
        };
        match fs::copy(&payload.source, &dst) {
            Ok(bytes) => Ok(UploadResponse {
                ok: true,
                bytes: bytes as f64,
                message: None,
            }),
            Err(e) => Ok(UploadResponse {
                ok: false,
                bytes: 0.0,
                message: Some(describe_io(&e)),
            }),
        }
    }

    pub fn mkdir(&self, payload: MkdirArgs) -> crate::Result<SimpleResponse> {
        let dir = match self.resolve(&payload.path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(SimpleResponse {
                    ok: false,
                    message: e.message,
                })
            }
        };
        match fs::create_dir_all(&dir) {
            Ok(()) => Ok(SimpleResponse {
                ok: true,
                message: None,
            }),
            Err(e) => Ok(SimpleResponse {
                ok: false,
                message: Some(describe_io(&e)),
            }),
        }
    }

    pub fn remove(&self, payload: RemoveArgs) -> crate::Result<SimpleResponse> {
        let target = match self.resolve(&payload.path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(SimpleResponse {
                    ok: false,
                    message: e.message,
                })
            }
        };
        let res = if payload.is_directory {
            fs::remove_dir_all(&target)
        } else {
            fs::remove_file(&target)
        };
        match res {
            Ok(()) => Ok(SimpleResponse {
                ok: true,
                message: None,
            }),
            Err(e) => Ok(SimpleResponse {
                ok: false,
                message: Some(describe_io(&e)),
            }),
        }
    }

    pub fn tailscale_status(&self) -> crate::Result<TailscaleStatus> {
        Ok(TailscaleStatus {
            installed: tailscale_exe().is_some(),
        })
    }

    pub fn open_tailscale(&self) -> crate::Result<OpenTailscaleResponse> {
        match tailscale_exe() {
            Some(exe) => match std::process::Command::new(&exe).spawn() {
                Ok(_) => Ok(OpenTailscaleResponse {
                    opened: true,
                    message: None,
                }),
                Err(e) => Ok(OpenTailscaleResponse {
                    opened: false,
                    message: Some(format!("No se pudo abrir Tailscale: {e}")),
                }),
            },
            None => Ok(OpenTailscaleResponse {
                opened: false,
                message: Some("No encuentro Tailscale instalado en este equipo".into()),
            }),
        }
    }

    /// En escritorio el selector de archivos devuelve rutas de verdad, así que
    /// el nombre es el último tramo. (En Android hay que preguntárselo al
    /// sistema porque las URIs `content://` son opacas.)
    pub fn uri_display_name(&self, payload: UriArgs) -> crate::Result<UriNameResponse> {
        let name = PathBuf::from(&payload.uri)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(UriNameResponse { name })
    }

    // ── Comandos de sincronización ──────────────────────────────────────────

    pub fn sync_read(&self, payload: SyncReadArgs) -> crate::Result<SyncReadResponse> {
        let path = match self.resolve(&payload.path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(SyncReadResponse {
                    ok: false,
                    code: e.code,
                    message: e.message,
                    content: None,
                })
            }
        };
        match read_capped(&path) {
            Ok(bytes) => {
                let content = if payload.binary {
                    base64_encode(&bytes)
                } else {
                    String::from_utf8_lossy(&bytes).to_string()
                };
                Ok(SyncReadResponse {
                    ok: true,
                    code: None,
                    message: None,
                    content: Some(content),
                })
            }
            Err(e) => Ok(SyncReadResponse {
                ok: false,
                code: Some(error_code(&e).into()),
                message: Some(describe_io(&e)),
                content: None,
            }),
        }
    }

    pub fn sync_write(&self, payload: SyncWriteArgs) -> crate::Result<SyncSimpleResponse> {
        let path = match self.resolve(&payload.path) {
            Ok(p) => p,
            Err(e) => return Ok(e),
        };
        let bytes = if payload.binary {
            match base64_decode(&payload.content) {
                Some(bytes) => bytes,
                None => {
                    return Ok(SyncSimpleResponse {
                        ok: false,
                        code: Some("UNKNOWN".into()),
                        message: Some("El contenido en base64 no es válido".into()),
                    })
                }
            }
        } else {
            payload.content.into_bytes()
        };
        match fs::write(&path, &bytes) {
            Ok(()) => Ok(ok_envelope()),
            Err(e) => Ok(io_envelope(&e)),
        }
    }

    pub fn sync_stat(&self, payload: SyncPathArgs) -> crate::Result<SyncStatResponse> {
        let path = match self.resolve(&payload.path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(SyncStatResponse {
                    ok: false,
                    code: e.code,
                    message: e.message,
                    size: 0.0,
                    is_directory: false,
                })
            }
        };
        match fs::metadata(&path) {
            Ok(meta) => Ok(SyncStatResponse {
                ok: true,
                code: None,
                message: None,
                size: if meta.is_dir() {
                    0.0
                } else {
                    meta.len() as f64
                },
                is_directory: meta.is_dir(),
            }),
            Err(e) => Ok(SyncStatResponse {
                ok: false,
                code: Some(error_code(&e).into()),
                message: Some(describe_io(&e)),
                size: 0.0,
                is_directory: false,
            }),
        }
    }

    pub fn sync_list(&self, payload: SyncPathArgs) -> crate::Result<SyncListResponse> {
        let dir = match self.resolve(&payload.path) {
            Ok(p) => p,
            Err(e) => {
                return Ok(SyncListResponse {
                    ok: false,
                    code: e.code,
                    message: e.message,
                    entries: Vec::new(),
                })
            }
        };
        match read_entries(&dir, &payload.path) {
            Ok(entries) => Ok(SyncListResponse {
                ok: true,
                code: None,
                message: None,
                entries,
            }),
            Err(e) => Ok(SyncListResponse {
                ok: false,
                code: Some(error_code(&e).into()),
                message: Some(describe_io(&e)),
                entries: Vec::new(),
            }),
        }
    }

    /// Idempotente: una carpeta que ya existe es un éxito.
    pub fn sync_mkdir(&self, payload: SyncPathArgs) -> crate::Result<SyncSimpleResponse> {
        let dir = match self.resolve(&payload.path) {
            Ok(p) => p,
            Err(e) => return Ok(e),
        };
        match fs::create_dir_all(&dir) {
            Ok(()) => Ok(ok_envelope()),
            Err(e) => Ok(io_envelope(&e)),
        }
    }

    /// Borrar algo que no está también es un éxito (lo pide el motor).
    pub fn sync_remove_dir(&self, payload: SyncPathArgs) -> crate::Result<SyncSimpleResponse> {
        let dir = match self.resolve(&payload.path) {
            Ok(p) => p,
            Err(e) => return Ok(e),
        };
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(ok_envelope()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(ok_envelope()),
            Err(e) => Ok(io_envelope(&e)),
        }
    }
}

// ---------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------

fn no_connection() -> SyncSimpleResponse {
    SyncSimpleResponse {
        ok: false,
        code: Some("NETWORK".into()),
        message: Some("No hay conexión activa con el NAS".into()),
    }
}

fn ok_envelope() -> SyncSimpleResponse {
    SyncSimpleResponse {
        ok: true,
        code: None,
        message: None,
    }
}

fn io_envelope(e: &io::Error) -> SyncSimpleResponse {
    SyncSimpleResponse {
        ok: false,
        code: Some(error_code(e).into()),
        message: Some(describe_io(e)),
    }
}

/// Lee un archivo pequeño (config.json, portadas) con tope de tamaño: por aquí
/// no debería pasar nada grande, y cargarlo entero en memoria tumbaría la app.
fn read_capped(path: &std::path::Path) -> io::Result<Vec<u8>> {
    let meta = fs::metadata(path)?;
    if meta.len() > MAX_SYNC_READ_BYTES {
        return Err(io::Error::other(
            "El archivo es demasiado grande para leerlo en memoria",
        ));
    }
    fs::read(path)
}

/// Entradas de una carpeta, con la ruta relativa al share que espera el frontend.
fn read_entries(dir: &std::path::Path, relative_base: &str) -> io::Result<Vec<SmbEntry>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        // Un archivo que desaparece entre el listado y el stat no debe tumbar
        // el listado entero.
        let Ok(meta) = entry.metadata() else { continue };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        out.push(SmbEntry {
            path: child_path(relative_base, &name),
            name,
            is_directory: meta.is_dir(),
            size: if meta.is_dir() {
                0.0
            } else {
                meta.len() as f64
            },
            modified,
        });
    }
    Ok(out)
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 a mano: son treinta líneas y ahorra una dependencia más que mantener.
fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(BASE64_ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            BASE64_ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits = 0;
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    for c in text.bytes() {
        if c == b'=' {
            break;
        }
        if c.is_ascii_whitespace() {
            continue;
        }
        let value = BASE64_ALPHABET.iter().position(|&a| a == c)? as u32;
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = r"\\nas\libros";

    #[test]
    fn resolve_path_joins_with_backslashes() {
        assert_eq!(
            resolve_path(ROOT, "epubs/tolkien/hobbit.epub").unwrap(),
            PathBuf::from(r"\\nas\libros\epubs\tolkien\hobbit.epub")
        );
    }

    #[test]
    fn resolve_path_of_the_share_root_is_the_root() {
        assert_eq!(resolve_path(ROOT, "").unwrap(), PathBuf::from(ROOT));
        assert_eq!(resolve_path(ROOT, "/").unwrap(), PathBuf::from(ROOT));
    }

    #[test]
    fn resolve_path_ignores_empty_and_dot_segments() {
        assert_eq!(
            resolve_path(ROOT, "/epubs//./hobbit.epub").unwrap(),
            PathBuf::from(r"\\nas\libros\epubs\hobbit.epub")
        );
    }

    #[test]
    fn resolve_path_refuses_to_escape_the_share() {
        assert!(resolve_path(ROOT, "../secretos").is_none());
        assert!(resolve_path(ROOT, "epubs/../../secretos").is_none());
        assert!(resolve_path(ROOT, "C:/Windows").is_none());
    }

    #[test]
    fn child_path_uses_forward_slashes_for_the_frontend() {
        assert_eq!(child_path("", "libro.epub"), "libro.epub");
        assert_eq!(child_path("epubs", "libro.epub"), "epubs/libro.epub");
        assert_eq!(child_path("/epubs/", "libro.epub"), "epubs/libro.epub");
    }

    #[test]
    fn io_errors_map_to_the_frontend_vocabulary() {
        assert_eq!(
            error_code(&io::Error::from(io::ErrorKind::NotFound)),
            "NOT_FOUND"
        );
        assert_eq!(
            error_code(&io::Error::from(io::ErrorKind::PermissionDenied)),
            "AUTH_FAILED"
        );
        assert_eq!(
            error_code(&io::Error::from(io::ErrorKind::ConnectionReset)),
            "NETWORK"
        );
    }

    #[test]
    fn wnet_logon_failure_explains_the_truenas_fix() {
        let message = describe_wnet_error(1326);
        assert!(message.contains("Samba Authentication"));
        // El otro código de contraseña incorrecta da el mismo consejo.
        assert_eq!(describe_wnet_error(86), message);
    }

    #[test]
    fn wnet_unknown_code_still_says_which_one() {
        assert!(describe_wnet_error(4321).contains("4321"));
    }

    #[test]
    fn base64_round_trips() {
        for value in ["", "h", "ho", "hol", "hola", "hola mundo\0\u{1}\u{2}"] {
            let encoded = base64_encode(value.as_bytes());
            assert_eq!(
                base64_decode(&encoded).unwrap(),
                value.as_bytes(),
                "{value}"
            );
        }
    }

    #[test]
    fn base64_encodes_the_expected_padding() {
        assert_eq!(base64_encode(b"hola"), "aG9sYQ==");
        assert_eq!(base64_encode(b"hol"), "aG9s");
    }

    #[test]
    fn base64_rejects_garbage() {
        assert!(base64_decode("!!!!").is_none());
    }
}
