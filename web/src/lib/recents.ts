import { useSyncExternalStore } from 'react';
import { readStored, writeStored } from './persist';

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

function parseList(raw: string | null): RecentNote[] {
  try {
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? (parsed as RecentNote[]) : [];
  } catch {
    // A corrupt preference is discarded, never surfaced as a crash.
    return [];
  }
}

// One module-level snapshot so every subscriber sees the same object and
// useSyncExternalStore can compare by identity.
let snapshot: RecentsSnapshot = {
  recents: parseList(readStored(RECENTS_KEY)),
  pins: parseList(readStored(PINS_KEY)),
};
const listeners = new Set<() => void>();

function commit(next: RecentsSnapshot): void {
  snapshot = next;
  writeStored(RECENTS_KEY, JSON.stringify(next.recents));
  writeStored(PINS_KEY, JSON.stringify(next.pins));
  listeners.forEach((listener) => listener());
}

function sameNote(note: RecentNote, other: { rootId: string; path: string }): boolean {
  return note.rootId === other.rootId && note.path === other.path;
}

export function recordOpen(rootId: string, path: string, title: string): void {
  const entry: RecentNote = { rootId, path, title, openedAt: Date.now() };
  const rest = snapshot.recents.filter((note) => !sameNote(note, entry));
  const sameRoot = rest.filter((note) => note.rootId === rootId).slice(0, MAX_RECENTS_PER_ROOT - 1);
  const otherRoots = rest.filter((note) => note.rootId !== rootId);
  const recents = [entry, ...sameRoot, ...otherRoots].sort((newer, older) => older.openedAt - newer.openedAt);
  // A pinned note keeps its title current too, since a rename changes it.
  const pins = snapshot.pins.map((note) => (sameNote(note, entry) ? { ...note, title } : note));
  commit({ recents, pins });
}

export function isPinned(rootId: string, path: string): boolean {
  return snapshot.pins.some((note) => sameNote(note, { rootId, path }));
}

export function togglePin(note: RecentNote): void {
  const pins = isPinned(note.rootId, note.path)
    ? snapshot.pins.filter((pinned) => !sameNote(pinned, note))
    : [...snapshot.pins, note];
  commit({ ...snapshot, pins });
}

/** Pinned notes first, then the rest of the recents, for one root or for all. */
export function listRecents(snap: RecentsSnapshot, rootId?: string): RecentNote[] {
  const inScope = (note: RecentNote) => rootId === undefined || note.rootId === rootId;
  const pinned = snap.pins.filter(inScope);
  const unpinned = snap.recents.filter((note) => inScope(note) && !snap.pins.some((pin) => sameNote(pin, note)));
  return [...pinned, ...unpinned];
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
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
