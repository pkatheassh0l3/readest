/**
 * Cliente del plugin `nas-smb`: acceso a una carpeta compartida por SMB/CIFS
 * (TrueNAS) y arranque de Tailscale.
 *
 * La parte nativa está en Kotlin con smbj (ver
 * `src-tauri/plugins/tauri-plugin-nas-smb`), y solo existe en Android: en
 * escritorio e iOS los comandos responden que no está disponible.
 */
import { invoke } from '@tauri-apps/api/core';

export interface NasCredentials {
  host: string;
  port?: number;
  share: string;
  username: string;
  password: string;
  domain?: string;
}

export interface SmbEntry {
  name: string;
  path: string;
  isDirectory: boolean;
  /** Tamaño en bytes. Viaja como número porque el puente con Kotlin es JSON. */
  size: number;
  /** Última modificación en milisegundos desde epoch; 0 si el NAS no la da. */
  modified: number;
}

export interface ConnectResult {
  ok: boolean;
  message?: string;
}

export interface TransferResult {
  ok: boolean;
  bytes: number;
  message?: string;
}

export interface SimpleResult {
  ok: boolean;
  message?: string;
}

export const smbConnect = async (credentials: NasCredentials): Promise<ConnectResult> =>
  invoke<ConnectResult>('plugin:nas-smb|connect', { payload: credentials });

export const smbList = async (path: string): Promise<SmbEntry[]> => {
  const res = await invoke<{ entries: SmbEntry[] }>('plugin:nas-smb|list', {
    payload: { path },
  });
  return res.entries ?? [];
};

/** Descarga un archivo del NAS a una ruta local absoluta. */
export const smbDownload = async (path: string, dest: string): Promise<TransferResult> =>
  invoke<TransferResult>('plugin:nas-smb|download', { payload: { path, dest } });

/** Sube un archivo local al NAS, sobrescribiendo si ya existe. */
export const smbUpload = async (source: string, path: string): Promise<TransferResult> =>
  invoke<TransferResult>('plugin:nas-smb|upload', { payload: { source, path } });

export const smbMkdir = async (path: string): Promise<SimpleResult> =>
  invoke<SimpleResult>('plugin:nas-smb|mkdir', { payload: { path } });

export const smbRemove = async (path: string, isDirectory: boolean): Promise<SimpleResult> =>
  invoke<SimpleResult>('plugin:nas-smb|remove', { payload: { path, isDirectory } });

export const smbDisconnect = async (): Promise<void> => {
  await invoke('plugin:nas-smb|disconnect');
};

export const isTailscaleInstalled = async (): Promise<boolean> => {
  const res = await invoke<{ installed: boolean }>('plugin:nas-smb|tailscale_status');
  return !!res.installed;
};

export const openTailscale = async (): Promise<{ opened: boolean; message?: string }> =>
  invoke<{ opened: boolean; message?: string }>('plugin:nas-smb|open_tailscale');

/**
 * Heurística para saber si el fallo es "no llego al NAS" (y entonces merece la
 * pena ofrecer Tailscale) o es otra cosa, como credenciales mal. Los mensajes
 * los genera el propio plugin en Kotlin.
 */
export const looksUnreachable = (message?: string): boolean => {
  if (!message) return false;
  const m = message.toLowerCase();
  return (
    m.includes('tailscale') ||
    m.includes('tiempo de espera') ||
    m.includes('sin ruta') ||
    m.includes('no se encuentra el host') ||
    m.includes('rechazada')
  );
};
