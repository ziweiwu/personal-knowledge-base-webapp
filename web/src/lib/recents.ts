import { useSyncExternalStore } from 'react';
import { readStored, storedKey, writeStored } from './persist';

/** A note the user opened, remembered so the sidebar and home page can offer it back. */
export interface RecentNote {
  rootId: string;
  path: string;
  title: string;
  openedAt: number;
}

export interface RecentsSnapshot {
  /** Newest first, capped per root. */
  recents: RecentNote[];
  /** Kept in the order they were pinned; never expire. */
  pins: RecentNote[];
}

const RECENTS_KEY = 'recents';
const PINS_KEY = 'pins';
const LAST_ROUTE_PREFIX = 'lastRoute.';
/** Enough to find yesterday's note without the list crowding out the tree. */
const MAX_RECENTS_PER_ROOT = 8;

/**
 * One entry, checked rather than merely cast.
 *
 * The array-ness was already checked and the elements were not, so an entry without a
 * `path` reached the route builder and threw — taking out the sidebar, and with it every
 * page, on every reload until site data was cleared. A stored preference is untrusted
 * input: the comment below has always promised it is discarded, and now it is.
 */
function parseNote(value: unknown): RecentNote | null {
  if (typeof value !== 'object' || value === null) return null;
  const { rootId, path, title, openedAt } = value as Partial<RecentNote>;
  if (typeof rootId !== 'string' || typeof path !== 'string') return null;
  if (rootId === '' || path === '') return null;
  return {
    rootId,
    path,
    title: typeof title === 'string' && title !== '' ? title : path,
    openedAt: typeof openedAt === 'number' ? openedAt : 0,
  };
}

function parseList(raw: string | null): RecentNote[] {
  try {
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    if (!Array.isArray(parsed)) return [];
    return parsed.map(parseNote).filter((note): note is RecentNote => note !== null);
  } catch {
    // A corrupt preference is discarded, never surfaced as a crash.
    return [];
  }
}

function readSnapshot(): RecentsSnapshot {
  return {
    recents: parseList(readStored(RECENTS_KEY)),
    pins: parseList(readStored(PINS_KEY)),
  };
}

// One module-level snapshot so every subscriber sees the same object and
// useSyncExternalStore can compare by identity.
let snapshot: RecentsSnapshot = readSnapshot();
const listeners = new Set<() => void>();

/**
 * Apply a change to what storage holds right now, not to what this tab last read.
 *
 * Both keys are written back whole, so a tab computing from a snapshot taken at import
 * silently discarded anything another tab had pinned since. This app is meant to be left
 * open all day in more than one tab, which makes that the ordinary case.
 */
function commit(update: (current: RecentsSnapshot) => RecentsSnapshot): void {
  const next = update(readSnapshot());
  snapshot = next;
  writeStored(RECENTS_KEY, JSON.stringify(next.recents));
  writeStored(PINS_KEY, JSON.stringify(next.pins));
  listeners.forEach((listener) => listener());
}

const WATCHED_KEYS = [storedKey(RECENTS_KEY), storedKey(PINS_KEY)];

/** Another tab wrote. A cleared store reports a null key and means both changed. */
function onStorageChanged(event: StorageEvent): void {
  if (event.key !== null && !WATCHED_KEYS.includes(event.key)) return;
  snapshot = readSnapshot();
  listeners.forEach((listener) => listener());
}

function sameNote(note: RecentNote, other: { rootId: string; path: string }): boolean {
  return note.rootId === other.rootId && note.path === other.path;
}

export function recordOpen(rootId: string, path: string, title: string): void {
  const entry: RecentNote = { rootId, path, title, openedAt: Date.now() };
  commit((current) => {
    const rest = current.recents.filter((note) => !sameNote(note, entry));
    const sameRoot = rest.filter((note) => note.rootId === rootId).slice(0, MAX_RECENTS_PER_ROOT - 1);
    const otherRoots = rest.filter((note) => note.rootId !== rootId);
    return {
      recents: [entry, ...sameRoot, ...otherRoots].sort((newer, older) => older.openedAt - newer.openedAt),
      // A pinned note keeps its title current too, since a rename changes it.
      pins: current.pins.map((note) => (sameNote(note, entry) ? { ...note, title } : note)),
    };
  });
}

export function isPinned(rootId: string, path: string): boolean {
  return snapshot.pins.some((note) => sameNote(note, { rootId, path }));
}

export function togglePin(note: RecentNote): void {
  commit((current) => ({
    ...current,
    pins: current.pins.some((pinned) => sameNote(pinned, note))
      ? current.pins.filter((pinned) => !sameNote(pinned, note))
      : [...current.pins, note],
  }));
}

/** Pinned notes first, then the rest of the recents, for one root or for all. */
export function listRecents(snap: RecentsSnapshot, rootId?: string): RecentNote[] {
  const inScope = (note: RecentNote) => rootId === undefined || note.rootId === rootId;
  const pinned = snap.pins.filter(inScope);
  const unpinned = snap.recents.filter((note) => inScope(note) && !snap.pins.some((pin) => sameNote(pin, note)));
  return [...pinned, ...unpinned];
}

function subscribe(listener: () => void): () => void {
  if (listeners.size === 0) window.addEventListener('storage', onStorageChanged);
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) window.removeEventListener('storage', onStorageChanged);
  };
}

function getSnapshot(): RecentsSnapshot {
  return snapshot;
}

export function useRecents(): RecentsSnapshot {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

/** The route to return to when this root is picked again; the listing until then. */
export function rememberLastRoute(rootId: string, route: string): void {
  writeStored(LAST_ROUTE_PREFIX + rootId, route);
}

export function lastRoute(rootId: string): string | null {
  return readStored(LAST_ROUTE_PREFIX + rootId);
}
