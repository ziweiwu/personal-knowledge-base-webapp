import type { DocumentKind } from '../api/types';

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];
const BYTES_PER_UNIT = 1024;
/** Below this many of a unit a fractional digit still carries information. */
const FRACTION_THRESHOLD = 100;

export function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '—';
  let value = bytes;
  let unit = 0;
  while (value >= BYTES_PER_UNIT && unit < UNITS.length - 1) {
    value /= BYTES_PER_UNIT;
    unit += 1;
  }
  const digits = unit === 0 || value >= FRACTION_THRESHOLD ? 0 : 1;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

const DATE_FORMAT = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' });
const DATE_TIME_FORMAT = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });
const RELATIVE_FORMAT = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;

export function formatDate(mtimeMs: number): string {
  if (!Number.isFinite(mtimeMs) || mtimeMs <= 0) return '—';
  return DATE_FORMAT.format(new Date(mtimeMs));
}

export function formatDateTime(mtimeMs: number): string {
  if (!Number.isFinite(mtimeMs) || mtimeMs <= 0) return '—';
  return DATE_TIME_FORMAT.format(new Date(mtimeMs));
}

/** "3 hours ago" for anything recent, an absolute date beyond a week. */
export function formatRelative(mtimeMs: number): string {
  if (!Number.isFinite(mtimeMs) || mtimeMs <= 0) return '—';
  const delta = mtimeMs - Date.now();
  const magnitude = Math.abs(delta);
  if (magnitude < MINUTE) return 'just now';
  if (magnitude < HOUR) return RELATIVE_FORMAT.format(Math.round(delta / MINUTE), 'minute');
  if (magnitude < DAY) return RELATIVE_FORMAT.format(Math.round(delta / HOUR), 'hour');
  if (magnitude < WEEK) return RELATIVE_FORMAT.format(Math.round(delta / DAY), 'day');
  return DATE_FORMAT.format(new Date(mtimeMs));
}

/** A folder is not a `DocumentKind`, but it sits in the same column of the UI. */
export type EntryKind = DocumentKind | 'folder';

const KIND_ICONS: Record<EntryKind, string> = {
  folder: '📁',
  markdown: '📝',
  docx: '📄',
  pdf: '📕',
  image: '🖼️',
  csv: '📊',
  text: '📃',
  binary: '📦',
};

const KIND_LABELS: Record<EntryKind, string> = {
  folder: 'Folder',
  markdown: 'Markdown document',
  docx: 'Word document',
  pdf: 'PDF',
  image: 'Image',
  csv: 'Spreadsheet',
  text: 'Text file',
  binary: 'File',
};

export function kindIcon(kind: EntryKind | undefined): string {
  return kind ? KIND_ICONS[kind] : '📄';
}

export function kindLabel(kind: EntryKind | undefined): string {
  return kind ? KIND_LABELS[kind] : 'File';
}
