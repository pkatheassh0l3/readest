package com.readest.nas_smb

import android.app.Activity
import android.util.Log
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.hierynomus.msdtyp.AccessMask
import com.hierynomus.msfscc.FileAttributes
import com.hierynomus.msfscc.fileinformation.FileIdBothDirectoryInformation
import com.hierynomus.mssmb2.SMB2CreateDisposition
import com.hierynomus.mssmb2.SMB2CreateOptions
import com.hierynomus.mssmb2.SMB2ShareAccess
import com.hierynomus.smbj.SMBClient
import com.hierynomus.smbj.SmbConfig
import com.hierynomus.smbj.auth.AuthenticationContext
import com.hierynomus.smbj.connection.Connection
import com.hierynomus.smbj.session.Session
import com.hierynomus.smbj.share.DiskShare
import org.json.JSONArray
import java.io.File
import java.io.FileOutputStream
import java.net.ConnectException
import java.net.NoRouteToHostException
import java.net.SocketTimeoutException
import java.net.UnknownHostException
import java.util.EnumSet
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

@InvokeArg
class ConnectArgs {
    var host: String = ""
    var port: Int? = null
    var share: String = ""
    var username: String = ""
    var password: String = ""
    var domain: String? = null
}

@InvokeArg
class ListArgs {
    var path: String = ""
}

@InvokeArg
class DownloadArgs {
    var path: String = ""
    var dest: String = ""
}

/**
 * Acceso SMB/CIFS (TrueNAS) para Readest.
 *
 * La lógica es la misma que ya funcionaba en NasConnect: una única conexión
 * viva, rutas normalizadas y, sobre todo, los mensajes de error traducidos a
 * algo accionable — los de TrueNAS en crudo (STATUS_LOGON_FAILURE y compañía)
 * no dicen nada sobre lo que hay que tocar en el NAS.
 *
 * Toda la E/S de red va en un hilo propio: en el principal Android lanzaría
 * NetworkOnMainThreadException, y además así las operaciones SMB quedan
 * serializadas (la sesión de smbj no es segura entre hilos).
 */
@TauriPlugin
class NasSmbPlugin(private val activity: Activity) : Plugin(activity) {

    companion object {
        private const val TAG = "NasSmbPlugin"
        private const val DEFAULT_PORT = 445
    }

    private val io = Executors.newSingleThreadExecutor { r -> Thread(r, "nas-smb") }

    private var client: SMBClient? = null
    private var connection: Connection? = null
    private var session: Session? = null
    private var diskShare: DiskShare? = null

    @Command
    fun connect(invoke: Invoke) {
        val args = invoke.parseArgs(ConnectArgs::class.java)
        io.execute {
            closeAll()
            try {
                val config = SmbConfig.builder()
                    // Tiempos cortos a propósito: así se nota enseguida que el NAS
                    // no es alcanzable (típicamente, Tailscale apagado) en vez de
                    // dejar la pantalla colgada medio minuto.
                    .withTimeout(6, TimeUnit.SECONDS)
                    .withSoTimeout(8, TimeUnit.SECONDS)
                    .build()
                val smbClient = SMBClient(config)
                val conn = smbClient.connect(args.host, args.port ?: DEFAULT_PORT)
                // Mandamos "" en vez de null cuando no hay dominio: algunas
                // configuraciones de Samba/TrueNAS responden distinto según ese
                // campo del mensaje NTLM, y "" es lo que mandan Windows y macOS
                // contra un servidor suelto (sin Active Directory).
                val auth = AuthenticationContext(
                    args.username,
                    args.password.toCharArray(),
                    args.domain ?: ""
                )
                val sess = conn.authenticate(auth)
                val share = sess.connectShare(args.share) as DiskShare

                client = smbClient
                connection = conn
                session = sess
                diskShare = share

                invoke.resolve(JSObject().put("ok", true))
            } catch (e: Exception) {
                Log.w(TAG, "connect falló", e)
                closeAll()
                invoke.resolve(
                    JSObject().put("ok", false).put("message", describeError(e))
                )
            }
        }
    }

    @Command
    fun list(invoke: Invoke) {
        val args = invoke.parseArgs(ListArgs::class.java)
        io.execute {
            val share = diskShare
            if (share == null) {
                invoke.reject("No hay conexión activa con el NAS")
                return@execute
            }
            try {
                val base = normalizePath(args.path)
                val raw: List<FileIdBothDirectoryInformation> = share.list(base)
                val entries = raw
                    .filter { it.fileName != "." && it.fileName != ".." }
                    .map { info ->
                        val isDir =
                            (info.fileAttributes and FileAttributes.FILE_ATTRIBUTE_DIRECTORY.value) != 0L
                        Triple(info, isDir, if (base.isEmpty()) info.fileName else "$base/${info.fileName}")
                    }
                    // Carpetas primero y por orden alfabético, como cualquier explorador.
                    .sortedWith(compareByDescending<Triple<FileIdBothDirectoryInformation, Boolean, String>> {
                        it.second
                    }.thenBy { it.first.fileName.lowercase() })

                val array = JSONArray()
                for ((info, isDir, path) in entries) {
                    val item = JSObject()
                    item.put("name", info.fileName)
                    item.put("path", path)
                    item.put("isDirectory", isDir)
                    item.put("size", info.endOfFile.toDouble())
                    item.put(
                        "modified",
                        runCatching { info.lastWriteTime.toDate().time.toDouble() }.getOrDefault(0.0)
                    )
                    array.put(item)
                }
                invoke.resolve(JSObject().put("entries", array))
            } catch (e: Exception) {
                Log.w(TAG, "list falló", e)
                invoke.reject(describeError(e))
            }
        }
    }

    @Command
    fun download(invoke: Invoke) {
        val args = invoke.parseArgs(DownloadArgs::class.java)
        io.execute {
            val share = diskShare
            if (share == null) {
                invoke.reject("No hay conexión activa con el NAS")
                return@execute
            }
            try {
                val destFile = File(args.dest)
                destFile.parentFile?.mkdirs()

                var total = 0L
                val remote = share.openFile(
                    normalizePath(args.path),
                    EnumSet.of(AccessMask.GENERIC_READ),
                    EnumSet.of(FileAttributes.FILE_ATTRIBUTE_NORMAL),
                    EnumSet.of(SMB2ShareAccess.FILE_SHARE_READ),
                    SMB2CreateDisposition.FILE_OPEN,
                    EnumSet.noneOf(SMB2CreateOptions::class.java)
                )
                remote.use { f ->
                    f.inputStream.use { input ->
                        FileOutputStream(destFile).use { output ->
                            val buffer = ByteArray(64 * 1024)
                            while (true) {
                                val read = input.read(buffer)
                                if (read <= 0) break
                                output.write(buffer, 0, read)
                                total += read
                            }
                            output.flush()
                        }
                    }
                }
                invoke.resolve(JSObject().put("ok", true).put("bytes", total.toDouble()))
            } catch (e: Exception) {
                Log.w(TAG, "download falló", e)
                runCatching { File(args.dest).delete() } // no dejamos medio archivo suelto
                invoke.resolve(
                    JSObject().put("ok", false).put("bytes", 0.0).put("message", describeError(e))
                )
            }
        }
    }

    @Command
    fun disconnect(invoke: Invoke) {
        io.execute {
            closeAll()
            invoke.resolve()
        }
    }

    private fun closeAll() {
        runCatching { diskShare?.close() }
        runCatching { session?.close() }
        runCatching { connection?.close() }
        runCatching { client?.close() }
        diskShare = null
        session = null
        connection = null
        client = null
    }

    private fun normalizePath(path: String): String =
        path.trim('/').replace("//", "/")

    /**
     * Traduce el error a algo que diga QUÉ tocar. Los códigos de TrueNAS en
     * crudo no ayudan: STATUS_LOGON_FAILURE, por ejemplo, suele significar que
     * hay que regenerar el hash de Samba del usuario, no que la contraseña esté
     * mal escrita.
     */
    private fun describeError(e: Exception): String {
        val raw = e.message.orEmpty()
        val upper = raw.uppercase()
        return when {
            e is UnknownHostException ->
                "No se encuentra el host \"${e.message}\". Revisa la dirección del NAS."
            e is NoRouteToHostException ->
                "Sin ruta de red hasta el NAS. ¿Está activo Tailscale?"
            e is ConnectException ->
                "Conexión rechazada. ¿El puerto SMB (445) está abierto?"
            e is SocketTimeoutException ->
                "Tiempo de espera agotado conectando al NAS. Puede que necesites Tailscale."
            "STATUS_LOGON_FAILURE" in upper ->
                "Usuario o contraseña incorrectos. En TrueNAS: edita el usuario, activa " +
                    "\"Samba Authentication\" y vuelve a guardar la contraseña (aunque ya " +
                    "estuviera activada, hazlo de nuevo para regenerar el hash SMB)."
            "STATUS_ACCESS_DENIED" in upper ->
                "Acceso denegado a este recurso compartido. Revisa en TrueNAS, en " +
                    "Sharing → SMB, que este usuario esté permitido en la ACL del share."
            "STATUS_BAD_NETWORK_NAME" in upper ->
                "El recurso compartido no existe con ese nombre. Comprueba el nombre exacto " +
                    "en TrueNAS → Sharing → Windows Shares (SMB)."
            "STATUS_USER_SESSION_DELETED" in upper || "STATUS_LOGON_TYPE_NOT_GRANTED" in upper ->
                "TrueNAS rechazó la sesión. Puede que el usuario no tenga permitido iniciar " +
                    "sesión por SMB."
            "STATUS_OBJECT_NAME_NOT_FOUND" in upper || "STATUS_OBJECT_PATH_NOT_FOUND" in upper ->
                "No se encuentra esa carpeta o archivo en el NAS."
            raw.isNotBlank() -> raw
            else -> "Error desconocido al hablar con el NAS"
        }
    }
}
