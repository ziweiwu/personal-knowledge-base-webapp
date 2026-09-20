import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useNavigate } from 'react-router-dom';
import { search } from '../../api/client';
import { docRoute } from '../../api/paths';
import type { RootInfo, SearchHit } from '../../api/types';
import { useBodyScrollLock } from '../../hooks/useBodyScrollLock';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useInertBackground } from '../../hooks/useInertBackground';
import { describeError } from '../../lib/errors';
import { kindIconName } from '../../lib/format';
import { readStoredOneOf, writeStored } from '../../lib/persist';
import { parseSnippet } from '../../lib/snippet';
import { useRoots } from '../../state/roots-context';
import { Button } from '../ui/Button';
import { Spinner } from '../ui/States';
import { wrapIndex } from '../../lib/wrapIndex';
import { Icon } from '../ui/Icon';

const DEBOUNCE_MS = 180;

/** Where a query runs: the collection the user is in, or every configured one. */
type SearchScope = 'root' | 'all';
const SCOPES: readonly SearchScope[] = ['root', 'all'];
const SCOPE_KEY = 'search-scope';

/** A hit remembers which collection it came from, since a fan-out mixes several. */
interface ScopedHit {
  rootId: string;
  rootName: string;
  hit: SearchHit;
}

interface SearchResult {
  key: string;
  hits: ScopedHit[];
  error: Error | null;
}

interface HitGroup {
  rootId: string;
  rootName: string;
  /** Index of the group's first hit in the flat list the keyboard walks. */
  start: number;
  hits: ScopedHit[];
}

/** Which collections a scope covers; the current one leads so its hits come first. */
function targetsFor(scope: SearchScope, current: RootInfo, roots: RootInfo[]): RootInfo[] {
  if (scope === 'root') return [current];
  return [current, ...roots.filter((root) => root.id !== current.id)];
}

async function searchAcross(targets: RootInfo[], query: string, signal: AbortSignal): Promise<ScopedHit[]> {
  const lists = await Promise.all(
    targets.map((root) =>
      search(root.id, query, signal).then((hits) => hits.map((hit) => ({ rootId: root.id, rootName: root.name, hit }))),
    ),
  );
  return lists.flat();
}

/** Hits arrive already ordered by collection, so grouping is a single pass. */
function groupByRoot(hits: ScopedHit[]): HitGroup[] {
  const groups: HitGroup[] = [];
  hits.forEach((scoped, index) => {
    const last = groups[groups.length - 1];
    if (last && last.rootId === scoped.rootId) last.hits.push(scoped);
    else groups.push({ rootId: scoped.rootId, rootName: scoped.rootName, start: index, hits: [scoped] });
  });
  return groups;
}

function toError(cause: unknown): Error {
  return cause instanceof Error ? cause : new Error(String(cause));
}

/**
 * Runs the debounced query and hands back only a result that belongs to what is
 * currently typed and scoped; anything else means a request is still on its way.
 */
function useSearchResults(targets: RootInfo[], scope: SearchScope, trimmed: string) {
  const [result, setResult] = useState<SearchResult | null>(null);
  const key = JSON.stringify([scope, targets[0]?.id ?? '', trimmed]);
  const settled = result && result.key === key ? result : null;

  useEffect(() => {
    if (!trimmed) return;
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      searchAcross(targets, trimmed, controller.signal)
        .then((hits) => setResult({ key, hits, error: null }))
        .catch((cause: unknown) => {
          if (controller.signal.aborted) return;
          setResult({ key, hits: [], error: toError(cause) });
        });
    }, DEBOUNCE_MS);

    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
    // `targets` is rebuilt every render; the key already captures what matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, trimmed]);

  return {
    key,
    hits: settled?.hits ?? [],
    error: settled?.error ?? null,
    loading: trimmed.length > 0 && settled === null,
  };
}

function Snippet({ snippet }: { snippet: string }) {
  return (
    <p className="palette__hit-snippet">
      {parseSnippet(snippet).map((part, index) =>
        part.match ? <mark key={index}>{part.text}</mark> : <span key={index}>{part.text}</span>,
      )}
    </p>
  );
}

interface ScopeToggleProps {
  scope: SearchScope;
  rootName: string;
  onChange: (scope: SearchScope) => void;
}

function ScopeToggle({ scope, rootName, onChange }: ScopeToggleProps) {
  return (
    <div className="palette__scope" role="group" aria-label="Search scope">
      <Button
        variant="ghost"
        className="palette__scope-btn"
        aria-pressed={scope === 'root'}
        onClick={() => onChange('root')}
      >
        This collection
      </Button>
      <Button
        variant="ghost"
        className="palette__scope-btn"
        aria-pressed={scope === 'all'}
        onClick={() => onChange('all')}
      >
        All collections
      </Button>
      <span className="palette__scope-name">{scope === 'root' ? rootName : 'Every collection'}</span>
    </div>
  );
}

interface HitRowProps {
  scoped: ScopedHit;
  active: boolean;
  optionId: string;
  onHover: () => void;
  onOpen: () => void;
}

function HitRow({ scoped, active, optionId, onHover, onOpen }: HitRowProps) {
  const { hit } = scoped;
  return (
    <li id={optionId} role="option" aria-selected={active}>
      <button
        type="button"
        tabIndex={-1}
        className={`palette__hit${active ? ' palette__hit--active' : ''}`}
        onMouseEnter={onHover}
        onClick={onOpen}
      >
        <span className="kind-icon" aria-hidden="true">
          <Icon name={kindIconName(hit.kind)} />
        </span>
        <span className="palette__hit-main">
          <span className="palette__hit-title">{hit.title || hit.path}</span>
          <span className="palette__hit-path">{hit.path}</span>
          <Snippet snippet={hit.snippet} />
        </span>
      </button>
    </li>
  );
}

interface ResultListProps {
  hits: ScopedHit[];
  grouped: boolean;
  activeIndex: number;
  listId: string;
  listRef: React.RefObject<HTMLUListElement | null>;
  onHover: (index: number) => void;
  onOpen: (scoped: ScopedHit) => void;
}

/**
 * One listbox either way. A fan-out wraps each collection's hits in a labelled
 * group so the heading is read once, while the flat option index the arrow keys
 * walk stays continuous across groups.
 */
function ResultList({ hits, grouped, activeIndex, listId, listRef, onHover, onOpen }: ResultListProps) {
  const row = (scoped: ScopedHit, index: number) => (
    <HitRow
      key={`${scoped.rootId}-${scoped.hit.path}-${index}`}
      scoped={scoped}
      active={index === activeIndex}
      optionId={`${listId}-option-${index}`}
      onHover={() => onHover(index)}
      onOpen={() => onOpen(scoped)}
    />
  );

  return (
    <ul className="palette__results" role="listbox" id={listId} aria-label="Search results" ref={listRef}>
      {grouped
        ? groupByRoot(hits).map((group) => (
            <li key={group.rootId} role="group" aria-labelledby={`${listId}-group-${group.rootId}`}>
              <p className="palette__group" id={`${listId}-group-${group.rootId}`}>
                {group.rootName}
              </p>
              <ul className="palette__group-list" role="presentation">
                {group.hits.map((scoped, offset) => row(scoped, group.start + offset))}
              </ul>
            </li>
          ))
        : hits.map(row)}
    </ul>
  );
}

function statusText(loading: boolean, trimmed: string, count: number): string {
  if (loading) return 'Searching…';
  if (!trimmed) return '';
  if (count === 0) return `No matches for ${trimmed}`;
  return `${count} ${count === 1 ? 'result' : 'results'} for ${trimmed}`;
}

interface SearchPaletteProps {
  rootId: string;
  rootName: string;
  onClose: () => void;
}

export function SearchPalette({ rootId, rootName, onClose }: SearchPaletteProps) {
  const [query, setQuery] = useState('');
  const [scope, setScope] = useState<SearchScope>(() => readStoredOneOf(SCOPE_KEY, SCOPES) ?? 'root');
  const [activeIndex, setActiveIndex] = useState(0);
  const { roots } = useRoots();

  const trimmed = query.trim();
  const current: RootInfo = roots.find((root) => root.id === rootId) ?? {
    id: rootId,
    name: rootName,
    obsidianMode: false,
    readOnly: false,
    documents: 0,
    lastModifiedMs: null,
  };
  const targets = targetsFor(scope, current, roots);
  const { key, hits, error, loading } = useSearchResults(targets, scope, trimmed);

  const [highlightKey, setHighlightKey] = useState('');
  if (highlightKey !== key) {
    setHighlightKey(key);
    setActiveIndex(0);
  }

  const panelRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const navigate = useNavigate();
  const listId = useId();

  // Same contract as <Modal>: this panel lives outside #root, so the page behind
  // it can be taken out of the accessibility tree. Declared before the focus
  // trap so its cleanup un-inerts before focus is handed back.
  useInertBackground();
  useFocusTrap(panelRef);
  useEscapeKey(onClose);
  useBodyScrollLock('locked');

  // Keep the highlighted row inside the scroll viewport as arrows move it.
  useEffect(() => {
    listRef.current?.querySelectorAll('[role="option"]')[activeIndex]?.scrollIntoView({ block: 'nearest' });
  }, [activeIndex]);

  const changeScope = useCallback((next: SearchScope) => {
    setScope(next);
    writeStored(SCOPE_KEY, next);
  }, []);

  const open = useCallback(
    (scoped: ScopedHit) => {
      onClose();
      void navigate(docRoute(scoped.rootId, scoped.hit.path));
    },
    [navigate, onClose],
  );

  // Bound on the panel rather than the input so it works from the toggle buttons too.
  const onPanelKeyDown = (event: React.KeyboardEvent) => {
    if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === 'f') {
      event.preventDefault();
      changeScope(scope === 'root' ? 'all' : 'root');
    }
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (hits.length === 0) return;
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setActiveIndex((index) => wrapIndex(index + 1, hits.length));
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setActiveIndex((index) => wrapIndex(index - 1, hits.length));
    } else if (event.key === 'Home') {
      event.preventDefault();
      setActiveIndex(0);
    } else if (event.key === 'End') {
      event.preventDefault();
      setActiveIndex(hits.length - 1);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      const scoped = hits[activeIndex];
      if (scoped) open(scoped);
    }
  };

  const showEmpty = trimmed.length > 0 && !loading && !error && hits.length === 0;
  const where = scope === 'root' ? rootName : 'all collections';

  return createPortal(
    <div
      className="palette"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className="palette__panel"
        role="dialog"
        aria-modal="true"
        aria-label={`Search ${where}`}
        ref={panelRef}
        onKeyDown={onPanelKeyDown}
      >
        <input
          className="palette__input"
          type="search"
          role="combobox"
          aria-expanded={hits.length > 0}
          aria-controls={listId}
          aria-activedescendant={hits.length > 0 ? `${listId}-option-${activeIndex}` : undefined}
          aria-label={`Search ${where}`}
          placeholder={`Search ${where}…`}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={onKeyDown}
          autoComplete="off"
          spellCheck={false}
          data-autofocus
        />

        <ScopeToggle scope={scope} rootName={rootName} onChange={changeScope} />

        {loading ? (
          <div className="state state--compact">
            <Spinner label="Searching" />
          </div>
        ) : null}

        {error ? (
          <div className="state state--compact" role="alert">
            <p className="state__title">{describeError(error).title}</p>
            <p className="state__detail">{describeError(error).detail}</p>
          </div>
        ) : null}

        {showEmpty ? (
          <div className="state state--compact">
            <p className="state__detail">No matches for “{trimmed}”.</p>
          </div>
        ) : null}

        {!trimmed && !loading ? (
          <div className="state state--compact">
            <p className="state__detail">Type to search titles and contents of {where}.</p>
          </div>
        ) : null}

        {/*
          A screen-reader user gets no feedback that a query returned nothing, or how many
          results arrived, unless it is announced: the listbox updating is silent until
          they navigate into it. WCAG 2.2 SC 4.1.3.
        */}
        <p className="sr-only" role="status" aria-live="polite">
          {statusText(loading, trimmed, hits.length)}
        </p>

        <ResultList
          hits={hits}
          grouped={scope === 'all'}
          activeIndex={activeIndex}
          listId={listId}
          listRef={listRef}
          onHover={setActiveIndex}
          onOpen={open}
        />

        <div className="palette__foot">
          <span>
            <kbd>↑</kbd> <kbd>↓</kbd> to move
          </span>
          <span>
            <kbd>Enter</kbd> to open
          </span>
          <span>
            <kbd>⌘⇧F</kbd> to switch scope
          </span>
          <span>
            <kbd>Esc</kbd> to close
          </span>
        </div>
      </div>
    </div>,
    document.body,
  );
}
