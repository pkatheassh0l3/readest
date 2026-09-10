/**
 * Perfiles de conexión al NAS.
 *
 * Llevan contraseña, así que se guardan con el almacén seguro que ya tiene
 * Readest (`set_secure_item` del plugin native-bridge, que en Android va contra
 * EncryptedSharedPreferences). Nunca en localStorage.
 *
 * Fuera de la app nativa (web/escritorio sin ese puente) simplemente no hay
 * perfiles guardados: es preferible eso a dejar credenciales en claro.
 */
import { clearSecureItem, getSecureItem, setSecureItem } from '@/utils/bridge';
import { isTauriAppPlatform } from '@/services/environment';
import type { NasCredentials } from './smbClient';

const PROFILES_KEY = 'nas_smb_profiles';
const LAST_USED_KEY = 'nas_smb_last_used';

export interface NasProfile extends NasCredentials {
  /** Nombre para mostrar; si está vacío se compone con host y share. */
  label?: string;
}

/** Identifica un perfil de forma estable, sin depender de su posición en la lista. */
export const profileKey = (p: NasCredentials): string =>
  `${p.host}|${p.port ?? 445}|${p.share}|${p.username}`;

export const profileLabel = (p: NasProfile): string =>
  p.label?.trim() ? p.label.trim() : `${p.host}/${p.share}`;

export const loadProfiles = async (): Promise<NasProfile[]> => {
  if (!isTauriAppPlatform()) return [];
  try {
    const res = await getSecureItem({ key: PROFILES_KEY });
    if (!res.value) return [];
    const parsed = JSON.parse(res.value);
    return Array.isArray(parsed) ? (parsed as NasProfile[]) : [];
  } catch {
    // Un JSON corrupto no debe impedir usar la app: se empieza de cero.
    return [];
  }
};

export const saveProfiles = async (profiles: NasProfile[]): Promise<void> => {
  if (!isTauriAppPlatform()) return;
  await setSecureItem({ key: PROFILES_KEY, value: JSON.stringify(profiles) });
};

/** Añade el perfil o actualiza el que ya existía con esos mismos datos de conexión. */
export const upsertProfile = async (profile: NasProfile): Promise<NasProfile[]> => {
  const profiles = await loadProfiles();
  const key = profileKey(profile);
  const idx = profiles.findIndex((p) => profileKey(p) === key);
  if (idx >= 0) profiles[idx] = profile;
  else profiles.push(profile);
  await saveProfiles(profiles);
  return profiles;
};

export const deleteProfile = async (profile: NasProfile): Promise<NasProfile[]> => {
  const key = profileKey(profile);
  const profiles = (await loadProfiles()).filter((p) => profileKey(p) !== key);
  await saveProfiles(profiles);
  const last = await getLastUsedKey();
  if (last === key) await clearSecureItem({ key: LAST_USED_KEY });
  return profiles;
};

export const setLastUsed = async (profile: NasCredentials): Promise<void> => {
  if (!isTauriAppPlatform()) return;
  await setSecureItem({ key: LAST_USED_KEY, value: profileKey(profile) });
};

const getLastUsedKey = async (): Promise<string | null> => {
  if (!isTauriAppPlatform()) return null;
  try {
    const res = await getSecureItem({ key: LAST_USED_KEY });
    return res.value ?? null;
  } catch {
    return null;
  }
};

/** El último perfil usado con éxito, si sigue guardado. */
export const loadLastUsedProfile = async (): Promise<NasProfile | null> => {
  const key = await getLastUsedKey();
  if (!key) return null;
  const profiles = await loadProfiles();
  return profiles.find((p) => profileKey(p) === key) ?? null;
};
