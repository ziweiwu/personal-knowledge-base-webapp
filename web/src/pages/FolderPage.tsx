import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { fetchFolder } from '../api/client';
import { eventTouches } from '../api/events';
import { docRoute, folderRoute } from '../api/paths';
import type { ChangeEvent, FolderEntry } from '../api/types';
import { HtmlContent } from '../components/content/HtmlContent';
import { Banner, EmptyState, ErrorState, LoadingState } from '../components/ui/States';
import { useAsyncResource } from '../hooks/useAsyncResource';
import { useChangeEvents } from '../hooks/useChangeEvents';
import { formatRelative, formatSize, kindIcon, kindLabel } from '../lib/format';
import { readStored, readStoredOneOf, writeStored } from '../lib/persist';
import { useFileActions } from '../state/file-actions-context';
import { useVault } from '../state/vault-context';

type SortKey = 'name' | 'modified' | 'size';

const SORT_KEYS = ['name', 'modified', 'size'] as const;

/** Sort is one preference for the whole collection; a filter belongs to its folder. */
const SORT_STORAGE_KEY = 'folder.sort';
const filterStorageKey = (rootId: string, path: string) => `folder.filter.${rootId}/${path}`;

const SORT_LABELS: Record<SortKey, string> = {
  name: 'Name',
  modified: 'Last modified',
  size: 'Size',
};

function sortEntries(entries: FolderEntry[], key: SortKey): FolderEntry[] {
  const sorted = [...entries];
  sorted.sort((a, b) => {
    // Folders always lead, whichever sort is active.
    if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
    if (key === 'modified') return b.mtimeMs - a.mtimeMs;
    if (key === 'size') return b.size - a.size;
    return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });
  });
  return sorted;
}

interface FolderPageProps {
  rootId: string;
  path: string;
  onTitleChange: (title: string) => void;
}

export function FolderPage({ rootId, path, onTitleChange }: FolderPageProps) {
  const { canEdit, root, localChangeToken } = useVault();
  const actions = useFileActions();
  const [sortKey, setSortKeyState] = useState<SortKey>(
    () => readStoredOneOf(SORT_STORAGE_KEY, SORT_KEYS) ?? 'name',
  );
  const setSortKey = useCallback((key: SortKey) => {
    setSortKeyState(key);
    writeStored(SORT_STORAGE_KEY, key);
  }, []);

  const [filter, setFilterState] = useState('');

  const load = useCallback((signal: AbortSignal) => fetchFolder(rootId, path, signal), [rootId, path]);
  const resource = useAsyncResource(load);
  const { data: listing, reload } = resource;

  // An upload or a delete made from this tab produces no change event we can see
  // — the stream drops our own echo — so the listing is refetched on the local
  // signal instead of waiting for a manual reload.
  const [seenChangeToken, setSeenChangeToken] = useState(localChangeToken);
  if (seenChangeToken !== localChangeToken) {
    setSeenChangeToken(localChangeToken);
    reload();
  }

  // A filter is remembered per folder, never globally: one folder's filter must never
  // silently hide a different folder's contents. Navigating swaps in that folder's own
  // filter (usually none), and the same key restores it after a browser restart.
  const folderKey = `${rootId}/${path}`;
  const setFilter = useCallback(
    (value: string) => {
      setFilterState(value);
      writeStored(filterStorageKey(rootId, path), value);
    },
    [rootId, path],
  );

  const [filteredKey, setFilteredKey] = useState<string | null>(null);
  if (filteredKey !== folderKey) {
    setFilteredKey(folderKey);
    setFilterState(readStored(filterStorageKey(rootId, path)) ?? '');
  }

  useEffect(() => {
    onTitleChange(listing?.name || root?.name || '');
  }, [listing?.name, root?.name, onTitleChange]);

  const onChange = useCallback(
    (event: ChangeEvent) => {
      if (eventTouches(event, rootId, path)) reload();
    },
    [rootId, path, reload],
  );
  useChangeEvents(onChange);

  const entries = useMemo(() => {
    if (!listing) return [];
    const needle = filter.trim().toLowerCase();
    const filtered = needle ? listing.entries.filter((entry) => entry.name.toLowerCase().includes(needle)) : listing.entries;
    return sortEntries(filtered, sortKey);
  }, [listing, filter, sortKey]);

  if (resource.error) return <ErrorState error={resource.error} onRetry={resource.reload} />;
  if (!listing) return resource.loading ? <LoadingState label="Loading folder…" /> : null;

  const index = listing.index;

  return (
    <div className="doc">
      {index ? (
        <div className="doc__inner">
          <p className="doc__title">{index.meta.title || listing.name}</p>
          {index.renderWarning ? <Banner tone="warning">{index.renderWarning}</Banner> : null}
          {index.html ? <HtmlContent html={index.html} rootId={rootId} docPath={index.meta.path} /> : null}
        </div>
      ) : (
        <div className="doc__inner">
          <h1 className="doc__title">{listing.name || root?.name || 'Folder'}</h1>
        </div>
      )}

      <div className="doc__inner">
        <h2 className="folder__section-title" id="folder-contents">
          {index ? 'Also in this folder' : 'Contents'}{' '}
          {entries.length === listing.entries.length
            ? `(${listing.entries.length})`
            : `(${entries.length} of ${listing.entries.length})`}
        </h2>

        <div className="folder__toolbar">
          <label className="sr-only" htmlFor="folder-filter">
            Filter this folder by name
          </label>
          <input
            id="folder-filter"
            className="folder__filter"
            type="search"
            placeholder="Filter by name…"
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
          />
          {filter ? (
            <button type="button" className="btn" onClick={() => setFilter('')}>
              Clear filter
            </button>
          ) : null}
          <label className="sr-only" htmlFor="folder-sort">
            Sort entries
          </label>
          <select
            id="folder-sort"
            className="select"
            value={sortKey}
            onChange={(event) => setSortKey(event.target.value as SortKey)}
          >
            {(Object.keys(SORT_LABELS) as SortKey[]).map((key) => (
              <option key={key} value={key}>
                Sort: {SORT_LABELS[key]}
              </option>
            ))}
          </select>

          {canEdit ? (
            <>
              <button type="button" className="btn" onClick={() => actions.newNote(path)}>
                New note
              </button>
              <button type="button" className="btn" onClick={() => actions.newFolder(path)}>
                New folder
              </button>
              <button type="button" className="btn" onClick={() => actions.upload(path)}>
                Upload
              </button>
            </>
          ) : null}
        </div>

        {entries.length === 0 ? (
          <EmptyState
            title={filter ? 'Nothing matches that filter' : 'This folder is empty'}
            detail={filter ? `No entry here contains “${filter.trim()}”.` : undefined}
          />
        ) : (
          <ul className="entries" aria-labelledby="folder-contents">
            {entries.map((entry) => (
              <li className="entries__item" key={entry.path}>
                <Link
                  className="entries__link"
                  to={entry.isDir ? folderRoute(rootId, entry.path) : docRoute(rootId, entry.path)}
                >
                  <span className="kind-icon" aria-hidden="true">
                    {kindIcon(entry.isDir ? 'folder' : entry.kind)}
                  </span>
                  <span className="entries__name">
                    {entry.name}
                    <span className="entries__sub">
                      {kindLabel(entry.isDir ? 'folder' : entry.kind)}
                      {entry.isDir && entry.childCount !== undefined
                        ? ` · ${entry.childCount} ${entry.childCount === 1 ? 'item' : 'items'}`
                        : ''}
                    </span>
                  </span>
                  <span className="entries__meta">
                    {entry.isDir ? '' : `${formatSize(entry.size)} · `}
                    {formatRelative(entry.mtimeMs)}
                  </span>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
