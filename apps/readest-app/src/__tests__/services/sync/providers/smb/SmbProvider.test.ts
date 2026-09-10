import { beforeEach, describe, expect, test, vi } from 'vitest';
import { runSemanticContract } from '../../file/providerSemanticContract';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const { createSmbProvider } = await import('@/services/sync/providers/smb/SmbProvider');

beforeEach(() => {
  invokeMock.mockReset();
});

/** Última ruta que el proveedor pidió al plugin, ya en el espacio del share. */
const lastPath = (): string => invokeMock.mock.calls.at(-1)?.[1]?.payload?.path;

runSemanticContract('SmbProvider', () => ({
  makeProvider: () => createSmbProvider(),
  stageAbsent: () => {
    invokeMock.mockResolvedValue({ ok: false, code: 'NOT_FOUND', message: 'no está' });
  },
  stageAuthFailure: () => {
    invokeMock.mockResolvedValue({ ok: false, code: 'AUTH_FAILED', message: 'credenciales' });
  },
}));

describe('SmbProvider — carpeta oculta', () => {
  test('la carpeta base Readest se traduce a .readest', async () => {
    invokeMock.mockResolvedValue({ ok: true, content: '{}' });
    await createSmbProvider().readText('/Readest/books/abc/config.json');
    expect(lastPath()).toBe('.readest/books/abc/config.json');
  });

  test('respeta un rootPath propio y no lleva barra inicial', async () => {
    invokeMock.mockResolvedValue({ ok: true, content: '{}' });
    await createSmbProvider({ rootPath: '/libros' }).readText('/libros/Readest/library.json');
    expect(lastPath()).toBe('libros/.readest/library.json');
  });

  test('las rutas ajenas al trazado pasan tal cual', async () => {
    invokeMock.mockResolvedValue({ ok: true, content: '' });
    await createSmbProvider().readText('/otra/cosa.txt');
    expect(lastPath()).toBe('otra/cosa.txt');
  });
});

describe('SmbProvider — operaciones', () => {
  test('list devuelve rutas en el espacio del motor, no del share', async () => {
    invokeMock.mockResolvedValue({
      ok: true,
      entries: [
        { name: 'abc', isDirectory: true, size: 0, modified: 0 },
        { name: 'library.json', isDirectory: false, size: 12, modified: 1700000000000 },
      ],
    });
    const entries = await createSmbProvider().list('/Readest');
    expect(entries).toEqual([
      {
        name: 'abc',
        path: '/Readest/abc',
        isDirectory: true,
        size: undefined,
        lastModified: undefined,
      },
      {
        name: 'library.json',
        path: '/Readest/library.json',
        isDirectory: false,
        size: 12,
        lastModified: new Date(1700000000000).toISOString(),
      },
    ]);
  });

  test('head devuelve el tamaño', async () => {
    invokeMock.mockResolvedValue({ ok: true, size: 512, isDirectory: false });
    expect(await createSmbProvider().head('/Readest/x')).toEqual({ size: 512 });
  });

  test('readBinary descodifica el base64 del plugin', async () => {
    invokeMock.mockResolvedValue({ ok: true, content: btoa('hola') });
    const buffer = await createSmbProvider().readBinary('/Readest/books/abc/cover.png');
    expect(new TextDecoder().decode(buffer!)).toBe('hola');
  });

  test('writeBinary manda base64 y marca binary', async () => {
    invokeMock.mockResolvedValue({ ok: true });
    const bytes = new TextEncoder().encode('hola');
    await createSmbProvider().writeBinary(
      '/Readest/books/abc/cover.png',
      bytes.buffer as ArrayBuffer,
    );
    expect(invokeMock.mock.calls.at(-1)?.[1]?.payload).toMatchObject({
      content: btoa('hola'),
      binary: true,
    });
  });

  test('ensureDir crea cada carpeta de arriba abajo', async () => {
    invokeMock.mockResolvedValue({ ok: true });
    await createSmbProvider().ensureDir(['/Readest', '/Readest/books', '/Readest/books/abc']);
    expect(invokeMock.mock.calls.map((c) => c[1]?.payload?.path)).toEqual([
      '.readest',
      '.readest/books',
      '.readest/books/abc',
    ]);
  });

  test('un fallo de red al escribir se traduce a FileSyncError NETWORK', async () => {
    invokeMock.mockResolvedValue({ ok: false, code: 'NETWORK', message: 'sin conexión' });
    const err = await createSmbProvider()
      .writeText('/Readest/library.json', '{}')
      .catch((e: unknown) => e);
    expect(err).toMatchObject({ name: 'FileSyncError', code: 'NETWORK', message: 'sin conexión' });
  });
});
