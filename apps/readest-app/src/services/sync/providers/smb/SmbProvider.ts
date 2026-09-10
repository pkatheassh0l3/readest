/**
 * Proveedor SMB para el motor de sincronización por archivos de Readest.
 *
 * Es el cuarto backend (después de WebDAV, Google Drive y S3) y el único que no
 * habla por HTTP: las operaciones van al plugin nativo `nas-smb`, que mantiene
 * la sesión SMB abierta contra el TrueNAS. Eso tiene dos consecuencias buenas:
 *
 *  - No hace falta ninguna nube ni cuenta: el progreso de lectura, los
 *    marcadores y las notas viajan por la misma carpeta compartida de la que
 *    salen los libros.
 *  - Tampoco hacen falta credenciales aquí. El explorador ya está conectado
 *    (y guarda su perfil en el almacén seguro), así que el proveedor reutiliza
 *    esa sesión. Si no hay ninguna, el plugin responde NETWORK y el motor lo
 *    reintenta en la siguiente pasada.
 *
 * La carpeta es oculta a propósito: el motor pide rutas bajo `<root>/Readest/`
 * y aquí ese primer tramo se traduce a `.readest`, de modo que en el NAS queda
 *
 *     <share>/.readest/library.json
 *     <share>/.readest/books/<hash>/config.json
 *
 * sin ensuciar la carpeta de libros y sin tocar el trazado "congelado" de
 * `file/layout.ts`, que es compartido por todos los backends.
 */
import { invoke } from '@tauri-apps/api/core';
import type { ProgressHandler } from '@/utils/transfer';
import { SYNC_BASE_DIR, normalizeRoot } from '@/services/sync/file/layout';
import {
  FileSyncError,
  type FileEntry,
  type FileHead,
  type FileSyncErrorCode,
  type FileSyncProvider,
} from '@/services/sync/file/provider';
import { smbDownload, smbUpload } from '@/services/nas/smbClient';

/** Nombre oculto que sustituye a `Readest` como carpeta base en el NAS. */
export const SMB_HIDDEN_BASE_DIR = '.readest';

export interface SmbProviderConfig {
  /** Carpeta del share bajo la que vive todo. Normalmente la raíz. */
  rootPath?: string;
}

/** Sobre que devuelven los comandos `sync_*` del plugin. */
interface SyncEnvelope {
  ok: boolean;
  code?: string;
  message?: string;
}

interface SyncReadEnvelope extends SyncEnvelope {
  content?: string;
}

interface SyncStatEnvelope extends SyncEnvelope {
  size?: number;
  isDirectory?: boolean;
}

interface SyncListEnvelope extends SyncEnvelope {
  entries?: { name: string; isDirectory: boolean; size: number; modified: number }[];
}

const ERROR_CODES: FileSyncErrorCode[] = [
  'AUTH_FAILED',
  'NOT_FOUND',
  'NETWORK',
  'CONFLICT',
  'UNKNOWN',
];

const toErrorCode = (code?: string): FileSyncErrorCode =>
  ERROR_CODES.find((c) => c === code) ?? 'UNKNOWN';

const fail = (res: SyncEnvelope, operation: string, path: string): FileSyncError =>
  new FileSyncError(
    res.message ?? `No se pudo ${operation} "${path}" en el NAS`,
    toErrorCode(res.code),
  );

/** true cuando el fallo es simplemente "eso no está ahí". */
const isMissing = (res: SyncEnvelope): boolean => res.code === 'NOT_FOUND';

const base64ToArrayBuffer = (base64: string): ArrayBuffer => {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
};

const arrayBufferToBase64 = (buffer: ArrayBuffer): string => {
  const bytes = new Uint8Array(buffer);
  // A trozos: `String.fromCharCode(...bytes)` con una portada entera revienta
  // la pila de argumentos.
  const CHUNK = 0x8000;
  let binary = '';
  for (let i = 0; i < bytes.length; i += CHUNK) {
    binary += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  }
  return btoa(binary);
};

export const createSmbProvider = (config: SmbProviderConfig = {}): FileSyncProvider => {
  const rootPath = normalizeRoot(config.rootPath);
  const engineBase = rootPath === '/' ? `/${SYNC_BASE_DIR}` : `${rootPath}/${SYNC_BASE_DIR}`;
  const smbBase =
    rootPath === '/' ? `/${SMB_HIDDEN_BASE_DIR}` : `${rootPath}/${SMB_HIDDEN_BASE_DIR}`;

  /**
   * Ruta del motor -> ruta dentro del share: se renombra el tramo base
   * (`Readest` -> `.readest`) y se quita la barra inicial, que SMB no usa.
   */
  const toSmbPath = (enginePath: string): string => {
    const p = enginePath.startsWith('/') ? enginePath : `/${enginePath}`;
    const mapped =
      p === engineBase || p.startsWith(`${engineBase}/`)
        ? `${smbBase}${p.slice(engineBase.length)}`
        : p;
    return mapped.replace(/^\/+/, '');
  };

  const readRaw = async (path: string, binary: boolean): Promise<string | null> => {
    const res = await invoke<SyncReadEnvelope>('plugin:nas-smb|sync_read', {
      payload: { path: toSmbPath(path), binary },
    });
    if (res.ok) return res.content ?? '';
    if (isMissing(res)) return null;
    throw fail(res, 'leer', path);
  };

  const write = async (path: string, content: string, binary: boolean): Promise<void> => {
    const res = await invoke<SyncEnvelope>('plugin:nas-smb|sync_write', {
      payload: { path: toSmbPath(path), content, binary },
    });
    if (!res.ok) throw fail(res, 'escribir', path);
  };

  return {
    rootPath,

    readText: (path) => readRaw(path, false),

    readBinary: async (path) => {
      const base64 = await readRaw(path, true);
      return base64 === null ? null : base64ToArrayBuffer(base64);
    },

    head: async (path): Promise<FileHead | null> => {
      const res = await invoke<SyncStatEnvelope>('plugin:nas-smb|sync_stat', {
        payload: { path: toSmbPath(path) },
      });
      if (res.ok) return { size: res.size };
      if (isMissing(res)) return null;
      throw fail(res, 'consultar', path);
    },

    list: async (path): Promise<FileEntry[]> => {
      const res = await invoke<SyncListEnvelope>('plugin:nas-smb|sync_list', {
        payload: { path: toSmbPath(path) },
      });
      if (!res.ok) throw fail(res, 'listar', path);
      const parent = path.replace(/\/+$/, '');
      // Las rutas se recomponen aquí, en el espacio de nombres del motor: las
      // que devuelve el plugin llevan ya el `.readest` traducido y el motor no
      // sabría qué hacer con ellas.
      return (res.entries ?? []).map((e) => ({
        name: e.name,
        path: `${parent}/${e.name}`,
        isDirectory: e.isDirectory,
        size: e.isDirectory ? undefined : e.size,
        lastModified: e.modified ? new Date(e.modified).toISOString() : undefined,
      }));
    },

    writeText: (path, body) => write(path, body, false),

    writeBinary: (path, body) => write(path, arrayBufferToBase64(body), true),

    ensureDir: async (paths) => {
      for (const path of paths) {
        const res = await invoke<SyncEnvelope>('plugin:nas-smb|sync_mkdir', {
          payload: { path: toSmbPath(path) },
        });
        if (!res.ok) throw fail(res, 'crear la carpeta', path);
      }
    },

    deleteDir: async (path) => {
      const res = await invoke<SyncEnvelope>('plugin:nas-smb|sync_remove_dir', {
        payload: { path: toSmbPath(path) },
      });
      if (!res.ok && !isMissing(res)) throw fail(res, 'borrar la carpeta', path);
    },

    // Los libros no pasan por memoria: van directos entre el disco del
    // dispositivo y el NAS con los mismos comandos que usa el explorador.
    uploadStream: async (remotePath, localPath) => {
      try {
        const res = await smbUpload(localPath, toSmbPath(remotePath));
        return !!res.ok;
      } catch {
        return false;
      }
    },

    downloadStream: async (remotePath, localPath, _onProgress?: ProgressHandler) => {
      try {
        const res = await smbDownload(toSmbPath(remotePath), localPath);
        return !!res.ok;
      } catch {
        return false;
      }
    },
  };
};
