/**
 * Small wrappers around `localStorage` for view preferences.
 *
 * Every access is guarded: storage throws outright in Safari private browsing and in
 * embedded webviews with site data disabled, and a lost preference is a far better
 * outcome than a blank page.
 */

const PREFIX = 'kbview.';

export function readStored(key: string): string | null {
  try {
    return localStorage.getItem(PREFIX + key);
  } catch {
    return null;
  }
}

export function writeStored(key: string, value: string | null): void {
  try {
    if (value === null || value === '') localStorage.removeItem(PREFIX + key);
    else localStorage.setItem(PREFIX + key, value);
  } catch {
    // A preference that does not survive a restart beats a crash.
  }
}

/** Read a stored value only if it is still one of the values the app understands. */
export function readStoredOneOf<T extends string>(key: string, allowed: readonly T[]): T | null {
  const stored = readStored(key);
  return allowed.includes(stored as T) ? (stored as T) : null;
}
