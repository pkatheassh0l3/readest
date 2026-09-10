'use client';

import clsx from 'clsx';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useRouter } from 'next/navigation';
import {
  MdArrowBack,
  MdArrowUpward,
  MdCreateNewFolder,
  MdDelete,
  MdFolder,
  MdInsertDriveFile,
  MdMenuBook,
  MdRefresh,
  MdSync,
  MdUpload,
} from 'react-icons/md';
import { useEnv } from '@/context/EnvContext';
import { useAuth } from '@/context/AuthContext';
import { useLibrary } from '@/hooks/useLibrary';
import { useTranslation } from '@/hooks/useTranslation';
import { useSettingsStore } from '@/store/settingsStore';
import { runFileLibrarySyncPass } from '@/services/sync/file/runLibrarySync';
import { useLibraryStore } from '@/store/libraryStore';
import { useDeviceControlStore } from '@/store/deviceStore';
import { isTauriAppPlatform } from '@/services/environment';
import { eventDispatcher } from '@/utils/event';
import { copyURIToPath } from '@/utils/bridge';
import { ingestFile } from '@/services/ingestService';
import { navigateToLibrary, navigateToReader } from '@/utils/nav';
import { isSupportedBookExt } from '@/components/settings/integrations/webdavBrowseUtils';
import {
  looksDisconnected,
  looksUnreachable,
  openTailscale,
  smbConnect,
  smbDownload,
  smbList,
  smbMkdir,
  smbRemove,
  smbUpload,
  uriDisplayName,
  type NasCredentials,
  type SmbEntry,
} from '@/services/nas/smbClient';
import {
  deleteProfile,
  loadLastUsedProfile,
  loadProfiles,
  profileKey,
  profileLabel,
  setLastUsed,
  upsertProfile,
  type NasProfile,
} from '@/services/nas/profiles';

type Status = 'idle' | 'connecting' | 'connected' | 'failed';

const EMPTY_FORM: NasCredentials = {
  host: '',
  port: 445,
  share: '',
  username: '',
  password: '',
  domain: '',
};

const formatSize = (bytes: number): string => {
  if (!bytes) return '';
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value < 10 && i > 0 ? value.toFixed(1) : Math.round(value)} ${units[i]}`;
};

const parentPath = (path: string): string =>
  path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '';

/**
 * Explorador del NAS por SMB: es la pantalla con la que arranca la app.
 *
 * Al tocar un libro, lo descarga, lo mete en la biblioteca con el mismo
 * `ingestFile` que usa Readest para cualquier otra importación, y salta a SU
 * lector. Es decir, el NAS es la puerta de entrada y el lector sigue siendo el
 * de Readest, sin tocarlo.
 */
const NasExplorer: React.FC = () => {
  const _ = useTranslation();
  const router = useRouter();
  const { envConfig, appService } = useEnv();
  const { user } = useAuth();
  const { settings, setSettings } = useSettingsStore();
  /**
   * Arranque de la app: su propio hook carga ajustes y biblioteca en los stores
   * y avisa cuando está listo.
   *
   * Es imprescindible, y de forma poco evidente. `Reader.tsx` solo se dibuja si
   * `libraryLoaded && settings.globalReadSettings`; si no, pinta un div vacío
   * (la pantalla en blanco). `useLibrary` se encarga de ambas cosas... pero se
   * salta TODA la inicialización si la biblioteca ya está marcada como cargada.
   * Por eso hay que usar este hook y no un `setLibrary` por nuestra cuenta: eso
   * marcaría la biblioteca como cargada sin haber cargado los ajustes, y el
   * lector se quedaría en blanco para siempre.
   */
  const { libraryLoaded: appReady } = useLibrary();
  const { acquireBackKeyInterception, releaseBackKeyInterception } = useDeviceControlStore();

  const [profiles, setProfiles] = useState<NasProfile[]>([]);
  const [form, setForm] = useState<NasCredentials>(EMPTY_FORM);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showPassword, setShowPassword] = useState(false);

  const [status, setStatus] = useState<Status>('idle');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [suggestTailscale, setSuggestTailscale] = useState(false);
  /** true mientras es el intento automático al abrir la app (no mostramos el error como si lo hubiera pedido ella). */
  const [autoConnecting, setAutoConnecting] = useState(false);

  /**
   * Nombre de la carpeta nueva. Va en un campo dentro de la propia pantalla
   * porque `window.prompt` NO funciona en el WebView de la app (igual que
   * `window.confirm`, según advierte su propio código en WebDAVBrowsePane).
   */
  const [newFolderName, setNewFolderName] = useState<string | null>(null);

  const [path, setPath] = useState('');
  const [entries, setEntries] = useState<SmbEntry[]>([]);
  const [listing, setListing] = useState(false);
  const [busyPath, setBusyPath] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);

  /**
   * Sincronización de la lectura por el propio NAS.
   *
   * Va contra una carpeta oculta `.readest` del share (ver
   * `services/sync/providers/smb/SmbProvider.ts`), con el mismo motor que usa
   * Readest para WebDAV o Drive: así viajan el punto de lectura, los marcadores
   * y las notas entre dispositivos sin pasar por ninguna nube.
   */
  const syncEnabled = settings.smb?.enabled ?? true;

  const setSyncEnabled = async (enabled: boolean) => {
    const current = useSettingsStore.getState().settings;
    const next = { ...current, smb: { ...current.smb, enabled } };
    setSettings(next);
    const service = appService ?? (await envConfig.getAppService());
    await service?.saveSettings(next);
  };

  const syncNow = useCallback(async () => {
    if (!(useSettingsStore.getState().settings.smb?.enabled ?? true)) return;
    setSyncing(true);
    try {
      await runFileLibrarySyncPass(envConfig, _);
    } catch (e) {
      console.warn('[nas] sincronización falló', e);
    } finally {
      setSyncing(false);
    }
  }, [envConfig, _]);

  /**
   * Credenciales del último intento (con éxito o sin él): sirven para
   * reintentar al volver de Tailscale sin que haya que rellenar el formulario.
   */
  const lastAttempt = useRef<NasCredentials | null>(null);
  /** Se pone a true al abrir Tailscale, para reintentar al volver a la app. */
  const pendingTailscaleRetry = useRef(false);

  const loadFolder = useCallback(async (target: string) => {
    setListing(true);
    try {
      const items = await smbList(target);
      setEntries(items);
      setPath(target);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      // Con el móvil, la sesión SMB se cae sola: basta con dejar la app en
      // segundo plano un rato o que se apague Tailscale. Reintentar el listado
      // no arregla nada, así que volvemos a la pantalla de conexión (que
      // reintenta con el mismo perfil) en vez de dejar un aviso y un explorador
      // que ya no responde.
      if (looksDisconnected(message)) {
        setStatus('failed');
        setErrorMessage(message);
        setSuggestTailscale(looksUnreachable(message));
        setEntries([]);
      }
      eventDispatcher.dispatch('toast', { type: 'error', message });
    } finally {
      setListing(false);
    }
  }, []);

  const connect = useCallback(
    async (credentials: NasCredentials, opts?: { remember?: boolean; auto?: boolean }) => {
      const { remember = true, auto = false } = opts ?? {};
      lastAttempt.current = credentials;
      setStatus('connecting');
      setErrorMessage(null);
      setSuggestTailscale(false);
      if (auto) setAutoConnecting(true);
      try {
        const res = await smbConnect(credentials);
        if (res.ok) {
          setStatus('connected');
          if (remember) {
            const saved = await upsertProfile(credentials as NasProfile);
            setProfiles(saved);
          }
          await setLastUsed(credentials);
          await loadFolder('');
          // Recién conectadas, es el momento de traer por dónde iba la lectura
          // en el otro dispositivo: hasta ahora el NAS no era alcanzable.
          void syncNow();
        } else {
          setStatus('failed');
          // En el intento automático no gritamos el error: se deja la pantalla
          // de conexión con los datos puestos para que ella decida.
          setErrorMessage(auto ? null : (res.message ?? 'No se pudo conectar'));
          setSuggestTailscale(looksUnreachable(res.message));
        }
      } catch (e) {
        setStatus('failed');
        const message = e instanceof Error ? e.message : String(e);
        setErrorMessage(auto ? null : message);
        setSuggestTailscale(looksUnreachable(message));
      } finally {
        setAutoConnecting(false);
      }
    },
    [loadFolder, syncNow],
  );

  // Al abrir la app: perfiles guardados y conexión al último usado.
  useEffect(() => {
    let cancelled = false;

    (async () => {
      const saved = await loadProfiles();
      if (cancelled) return;
      setProfiles(saved);
      const last = await loadLastUsedProfile();
      if (cancelled) return;
      if (last) {
        setForm({ ...last });
        await connect(last, { remember: false, auto: true });
      }
    })();

    return () => {
      cancelled = true;
    };
    // Solo al montar: connect es estable y no queremos reconectar en cada render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /**
   * Reintento al volver de Tailscale.
   *
   * Aquí no hace falta pelearse con Android para traer la app al primer plano
   * (que es lo que no había manera de conseguir en la versión de Kotlin):
   * simplemente, cuando la usuaria vuelve por su cuenta, el WebView recibe el
   * evento de visibilidad y reintentamos en ese momento.
   */
  useEffect(() => {
    const retryIfPending = () => {
      if (!pendingTailscaleRetry.current) return;
      pendingTailscaleRetry.current = false;
      const credentials = lastAttempt.current;
      if (credentials) void connect(credentials, { remember: false });
    };
    const onVisible = () => {
      if (document.visibilityState !== 'visible') return;
      retryIfPending();
    };
    // Se escuchan los dos eventos, como hace su `useReplicaPull`: al volver al
    // primer plano no siempre llega `visibilitychange`, y cuando llega puede
    // hacerlo bastante después de `focus`.
    document.addEventListener('visibilitychange', onVisible);
    window.addEventListener('focus', retryIfPending);
    return () => {
      document.removeEventListener('visibilitychange', onVisible);
      window.removeEventListener('focus', retryIfPending);
    };
  }, [connect]);

  /**
   * Botón "atrás" de Android.
   *
   * Sin esto, atrás cierra la app. Lo que ella pidió es que suba una carpeta,
   * como en cualquier explorador. Se usa el mismo mecanismo que su
   * `useKeyDownActions`: se pide la intercepción (contada por referencias, se
   * suelta al desmontar) y se escucha `native-key-down` de forma síncrona, que
   * es la única manera de consumir la tecla antes de que Android cierre.
   *
   * Devolver `true` = consumida; `false` = que siga su curso (y en la raíz,
   * salir de la app, que es lo esperable).
   */
  const backKeyState = useRef({ path, status, newFolderName });
  backKeyState.current = { path, status, newFolderName };

  useEffect(() => {
    if (!appService?.isAndroidApp) return;

    const handleBackKey = (event: CustomEvent) => {
      if (event.detail?.keyName !== 'Back') return false;
      const {
        path: currentPath,
        status: currentStatus,
        newFolderName: pendingName,
      } = backKeyState.current;
      // Primero se cierra el campo de "carpeta nueva" si está abierto.
      if (pendingName !== null) {
        setNewFolderName(null);
        return true;
      }
      if (currentStatus === 'connected' && currentPath) {
        void loadFolder(parentPath(currentPath));
        return true;
      }
      return false;
    };

    acquireBackKeyInterception?.();
    eventDispatcher.onSync('native-key-down', handleBackKey);
    return () => {
      releaseBackKeyInterception?.();
      eventDispatcher.offSync('native-key-down', handleBackKey);
    };
    // El manejador lee el estado por referencia, así que no hace falta volver a
    // registrarlo (ni soltar y pedir la intercepción) en cada navegación.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [appService?.isAndroidApp]);

  const handleOpenTailscale = async () => {
    const res = await openTailscale();
    if (!res.opened) {
      eventDispatcher.dispatch('toast', {
        type: 'error',
        message: res.message ?? 'No se pudo abrir Tailscale',
      });
      return;
    }
    pendingTailscaleRetry.current = true;
    eventDispatcher.dispatch('toast', {
      type: 'info',
      message: 'Activa Tailscale y vuelve aquí: reintentaré la conexión solo.',
    });
  };

  /** Descarga el libro, lo importa a la biblioteca y abre el lector de Readest. */
  const openBook = async (entry: SmbEntry) => {
    if (!isTauriAppPlatform()) return;
    const appService = await envConfig.getAppService();
    if (!appService) return;
    if (!appReady) {
      // Abrir el lector antes de que los ajustes estén en su store da pantalla
      // en blanco, así que preferimos avisar y que lo intente otra vez.
      eventDispatcher.dispatch('toast', {
        type: 'info',
        message: 'La app aún está arrancando; inténtalo de nuevo en un segundo.',
      });
      return;
    }

    setBusyPath(entry.path);
    try {
      const safeName = entry.name.replaceAll(/[/\\:*?"<>|]/g, '_').slice(0, 200) || 'download';
      const dst = await appService.resolveFilePath(`nas-${Date.now()}-${safeName}`, 'Cache');
      const res = await smbDownload(entry.path, dst);
      if (!res.ok) throw new Error(res.message ?? 'No se pudo descargar del NAS');

      const { library: storeLibrary, libraryLoaded, setLibrary } = useLibraryStore.getState();
      const library = libraryLoaded ? [...storeLibrary] : await appService.loadLibraryBooks();
      // Los ajustes se leen del store en este momento (no de una copia
      // capturada al renderizar), para que sean los ya cargados del disco.
      const currentSettings = useSettingsStore.getState().settings;
      const imported = await ingestFile(
        { file: dst, books: library },
        { appService, settings: currentSettings, isLoggedIn: !!user },
      );
      // ingestFile ya ha copiado los bytes a Books/<hash>/: la copia en caché
      // sobra. Si el borrado falla no pasa nada, lo limpia el sistema.
      try {
        await appService.deleteFile(dst, 'None');
      } catch {
        /* borrar la caché no es crítico */
      }
      if (!imported) throw new Error('La importación no devolvió libro');

      await appService.saveLibraryBooks(library);
      setLibrary(library);
      navigateToReader(router, [imported.hash]);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      // Igual que al listar: si la sesión SMB ya no vale, volvemos a la
      // pantalla de conexión en vez de dejar un aviso sin salida.
      if (looksDisconnected(message)) {
        setStatus('failed');
        setErrorMessage(message);
        setSuggestTailscale(looksUnreachable(message));
      }
      eventDispatcher.dispatch('toast', { type: 'error', message });
    } finally {
      setBusyPath(null);
    }
  };

  const handleEntryClick = (entry: SmbEntry) => {
    if (entry.isDirectory) {
      void loadFolder(entry.path);
      return;
    }
    if (isSupportedBookExt(entry.name)) {
      void openBook(entry);
      return;
    }
    eventDispatcher.dispatch('toast', {
      type: 'info',
      message: 'Ese archivo no es un libro que la app pueda abrir.',
    });
  };

  const handleUpload = async () => {
    const appService = await envConfig.getAppService();
    if (!appService) return;
    try {
      const { open: openDialog } = await import('@tauri-apps/plugin-dialog');
      const selected = await openDialog({ multiple: true });
      if (!selected) return;
      const picked = Array.isArray(selected) ? selected : [selected];

      for (const item of picked) {
        const uri = typeof item === 'string' ? item : String(item);
        // En Android el selector devuelve URIs content:// opacas: su último
        // segmento es algo como `msf%3A1000000033`, no un nombre de archivo.
        // Hay que preguntarle al sistema el nombre real, o el archivo llegaría
        // al NAS con ese churro y encima sin extensión.
        let name = '';
        if (uri.startsWith('content://')) {
          name = await uriDisplayName(uri).catch(() => '');
        }
        if (!name) name = uri.split(/[/\\]/).pop() || '';
        name = name.replaceAll(/[/\\:*?"<>|]/g, '_').trim() || `subida-${Date.now()}`;

        // El lado nativo de SMB abre archivos, no URIs: las content:// se pasan
        // antes a caché con el ayudante que ya trae Readest.
        let localPath = uri;
        if (uri.startsWith('content://')) {
          const dst = await appService.resolveFilePath(`upload-${Date.now()}-${name}`, 'Cache');
          const copied = await copyURIToPath({ uri, dst });
          if (!copied.success)
            throw new Error(copied.error ?? 'No se pudo leer el archivo elegido');
          localPath = dst;
        }
        const remote = path ? `${path}/${name}` : name;
        const res = await smbUpload(localPath, remote);
        if (!res.ok) throw new Error(res.message ?? `No se pudo subir ${name}`);
      }
      eventDispatcher.dispatch('toast', { type: 'info', message: 'Subida completada.' });
      await loadFolder(path);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      eventDispatcher.dispatch('toast', { type: 'error', message });
    }
  };

  const handleCreateFolder = async () => {
    const name = (newFolderName ?? '').trim();
    if (!name) return;
    const target = path ? `${path}/${name}` : name;
    const res = await smbMkdir(target);
    if (!res.ok) {
      eventDispatcher.dispatch('toast', {
        type: 'error',
        message: res.message ?? 'No se pudo crear la carpeta',
      });
      return;
    }
    setNewFolderName(null);
    await loadFolder(path);
  };

  const handleDelete = async (entry: SmbEntry) => {
    const appService = await envConfig.getAppService();
    if (!appService) return;
    // appService.ask, NO window.confirm: en el WebView de la app el segundo no
    // hace nada (lo advierte su propio WebDAVBrowsePane), así que el borrado se
    // ejecutaría sin que llegaras a ver la pregunta.
    const ok = await appService.ask(`¿Borrar "${entry.name}" del NAS?`);
    if (!ok) return;
    const res = await smbRemove(entry.path, entry.isDirectory);
    if (!res.ok) {
      eventDispatcher.dispatch('toast', {
        type: 'error',
        message: res.message ?? 'No se pudo borrar',
      });
      return;
    }
    setNewFolderName(null);
    await loadFolder(path);
  };

  const handleDeleteProfile = async (profile: NasProfile) => {
    const remaining = await deleteProfile(profile);
    setProfiles(remaining);
  };

  // ── Pantalla de conexión ────────────────────────────────────────────────
  if (status !== 'connected') {
    return (
      <div className='mx-auto flex min-h-screen w-full max-w-md flex-col gap-4 p-4'>
        <h1 className='text-xl font-bold'>{_('Conectar con el NAS')}</h1>

        {autoConnecting && (
          <div className='flex items-center gap-2 text-sm opacity-70'>
            <span className='loading loading-spinner loading-sm' />
            <span>{_('Conectando con tu último NAS…')}</span>
          </div>
        )}

        {profiles.length > 0 && (
          <div className='flex flex-col gap-2'>
            <span className='text-sm font-semibold opacity-70'>{_('Perfiles guardados')}</span>
            {profiles.map((p) => (
              <div key={profileKey(p)} className='flex items-center gap-2'>
                <button
                  className='btn btn-sm flex-1 justify-start'
                  onClick={() => {
                    setForm({ ...p });
                    void connect(p, { remember: false });
                  }}
                >
                  {profileLabel(p)}
                </button>
                <button
                  className='btn btn-sm btn-ghost'
                  aria-label={_('Borrar perfil')}
                  onClick={() => void handleDeleteProfile(p)}
                >
                  <MdDelete />
                </button>
              </div>
            ))}
          </div>
        )}

        <div className='flex flex-col gap-2'>
          <input
            className='input input-bordered input-sm'
            placeholder={_('Dirección del NAS (IP o nombre)')}
            value={form.host}
            onChange={(e) => setForm({ ...form, host: e.target.value })}
          />
          <input
            className='input input-bordered input-sm'
            placeholder={_('Recurso compartido (share)')}
            value={form.share}
            onChange={(e) => setForm({ ...form, share: e.target.value })}
          />
          <input
            className='input input-bordered input-sm'
            placeholder={_('Usuario')}
            autoCapitalize='none'
            value={form.username}
            onChange={(e) => setForm({ ...form, username: e.target.value })}
          />
          <div className='flex gap-2'>
            <input
              className='input input-bordered input-sm flex-1'
              type={showPassword ? 'text' : 'password'}
              placeholder={_('Contraseña')}
              value={form.password}
              onChange={(e) => setForm({ ...form, password: e.target.value })}
            />
            <button className='btn btn-sm' onClick={() => setShowPassword((v) => !v)}>
              {showPassword ? _('Ocultar') : _('Ver')}
            </button>
          </div>

          <button
            className='btn btn-ghost btn-xs self-start'
            onClick={() => setShowAdvanced((v) => !v)}
          >
            {_('Opciones avanzadas (puerto y dominio)')}
          </button>
          {showAdvanced && (
            <div className='flex gap-2'>
              <input
                className='input input-bordered input-sm w-24'
                type='number'
                placeholder='445'
                value={form.port ?? 445}
                onChange={(e) => setForm({ ...form, port: Number(e.target.value) || 445 })}
              />
              <input
                className='input input-bordered input-sm flex-1'
                placeholder={_('Dominio / workgroup')}
                value={form.domain ?? ''}
                onChange={(e) => setForm({ ...form, domain: e.target.value })}
              />
            </div>
          )}
        </div>

        <button
          className='btn btn-primary btn-sm'
          disabled={status === 'connecting' || !form.host || !form.share}
          onClick={() => void connect(form)}
        >
          {status === 'connecting' ? _('Conectando…') : _('Conectar')}
        </button>

        {errorMessage && (
          <div className='alert alert-error text-sm'>
            <span>{errorMessage}</span>
          </div>
        )}

        {suggestTailscale && (
          <div className='flex flex-col gap-2 rounded-lg border border-base-300 p-3 text-sm'>
            <span>{_('No llego al NAS. Si estás fuera de casa, necesitas Tailscale activo.')}</span>
            <button className='btn btn-sm' onClick={() => void handleOpenTailscale()}>
              {_('Abrir Tailscale')}
            </button>
          </div>
        )}

        <button className='btn btn-ghost btn-sm mt-auto' onClick={() => navigateToLibrary(router)}>
          {_('Ir a mi biblioteca')}
        </button>
      </div>
    );
  }

  // ── Explorador ──────────────────────────────────────────────────────────
  return (
    <div className='flex min-h-screen flex-col'>
      <div className='flex items-center gap-1 border-b border-base-300 p-2'>
        <button
          className='btn btn-ghost btn-sm'
          disabled={!path}
          aria-label={_('Subir un nivel')}
          onClick={() => void loadFolder(parentPath(path))}
        >
          <MdArrowUpward />
        </button>
        <span className='flex-1 truncate text-sm'>{path || _('Raíz del NAS')}</span>
        <button
          className='btn btn-ghost btn-sm'
          aria-label={_('Actualizar')}
          onClick={() => void loadFolder(path)}
        >
          <MdRefresh />
        </button>
        <button
          className='btn btn-ghost btn-sm'
          aria-label={_('Nueva carpeta')}
          onClick={() => setNewFolderName((v) => (v === null ? '' : null))}
        >
          <MdCreateNewFolder />
        </button>
        <button
          className='btn btn-ghost btn-sm'
          aria-label={_('Subir archivos')}
          onClick={() => void handleUpload()}
        >
          <MdUpload />
        </button>
        <button
          className='btn btn-ghost btn-sm'
          aria-label={_('Mi biblioteca')}
          onClick={() => navigateToLibrary(router)}
        >
          <MdMenuBook />
        </button>
      </div>

      {newFolderName !== null && (
        <div className='border-base-300 flex items-center gap-2 border-b p-2'>
          <input
            className='input input-bordered input-sm eink-bordered flex-1'
            autoFocus
            placeholder={_('Nombre de la nueva carpeta')}
            value={newFolderName}
            onChange={(e) => setNewFolderName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void handleCreateFolder();
              if (e.key === 'Escape') setNewFolderName(null);
            }}
          />
          <button
            className='btn btn-sm btn-contrast'
            disabled={!newFolderName.trim()}
            onClick={() => void handleCreateFolder()}
          >
            {_('Crear')}
          </button>
          <button className='btn btn-sm btn-ghost' onClick={() => setNewFolderName(null)}>
            {_('Cancelar')}
          </button>
        </div>
      )}

      {listing && (
        <div className='flex items-center gap-2 p-3 text-sm opacity-70'>
          <span className='loading loading-spinner loading-sm' />
          <span>{_('Leyendo carpeta…')}</span>
        </div>
      )}

      {!listing && entries.length === 0 && (
        <div className='p-6 text-center text-sm opacity-60'>{_('Esta carpeta está vacía.')}</div>
      )}

      <ul className='flex-1 divide-y divide-base-200'>
        {entries.map((entry) => {
          const isBook = !entry.isDirectory && isSupportedBookExt(entry.name);
          return (
            <li key={entry.path} className='flex items-center gap-2 px-3 py-2'>
              <button
                className='flex min-w-0 flex-1 items-center gap-3 text-left'
                onClick={() => handleEntryClick(entry)}
              >
                <span
                  className={clsx(
                    'text-xl',
                    entry.isDirectory ? 'text-primary' : isBook ? 'text-secondary' : 'opacity-50',
                  )}
                >
                  {entry.isDirectory ? (
                    <MdFolder />
                  ) : isBook ? (
                    <MdMenuBook />
                  ) : (
                    <MdInsertDriveFile />
                  )}
                </span>
                <span className='min-w-0 flex-1'>
                  <span className='block truncate text-sm'>{entry.name}</span>
                  {!entry.isDirectory && (
                    <span className='block text-xs opacity-60'>{formatSize(entry.size)}</span>
                  )}
                </span>
                {busyPath === entry.path && <span className='loading loading-spinner loading-xs' />}
              </button>
              <button
                className='btn btn-ghost btn-xs'
                aria-label={_('Borrar')}
                onClick={() => void handleDelete(entry)}
              >
                <MdDelete />
              </button>
            </li>
          );
        })}
      </ul>

      <div className='flex items-center gap-2 border-t border-base-300 p-2 text-sm'>
        <label className='flex flex-1 items-center gap-2'>
          <input
            type='checkbox'
            className='toggle toggle-sm eink-bordered'
            checked={syncEnabled}
            onChange={(e) => void setSyncEnabled(e.target.checked)}
          />
          <span>{_('Sincronizar la lectura por el NAS')}</span>
        </label>
        <button
          className='btn btn-ghost btn-sm'
          disabled={!syncEnabled || syncing}
          aria-label={_('Sincronizar ahora')}
          onClick={() => void syncNow()}
        >
          {syncing ? <span className='loading loading-spinner loading-xs' /> : <MdSync />}
        </button>
      </div>

      <div className='border-t border-base-300 p-2'>
        <button
          className='btn btn-ghost btn-sm'
          onClick={() => {
            setStatus('idle');
            setEntries([]);
            setPath('');
          }}
        >
          <MdArrowBack /> {_('Cambiar de NAS')}
        </button>
      </div>
    </div>
  );
};

export default NasExplorer;
