import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useNavigate } from 'react-router-dom';
import { search } from '../../api/client';
import { docRoute } from '../../api/paths';
import type { SearchHit } from '../../api/types';
import { useBodyScrollLock } from '../../hooks/useBodyScrollLock';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { kindIcon } from '../../lib/format';
import { parseSnippet } from '../../lib/snippet';
import { Spinner } from '../ui/States';
import { describeError } from '../../lib/errors';

const DEBOUNCE_MS = 180;

interface SearchResult {
  rootId: string;
  query: string;
  hits: SearchHit[];
  error: Error | null;
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

interface SearchPaletteProps {
  rootId: string;
  rootName: string;
  onClose: () => void;
}

export function SearchPalette({ rootId, rootName, onClose }: SearchPaletteProps) {
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<SearchResult | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);

  const trimmed = query.trim();
  // The result is only shown when it belongs to what is currently typed;
  // anything else means a request is still on its way.
  const settled = result && result.query === trimmed && result.rootId === rootId ? result : null;
  const hits = settled?.hits ?? [];
  const error = settled?.error ?? null;
  const loading = trimmed.length > 0 && settled === null;

  const [highlightKey, setHighlightKey] = useState('');
  const currentKey = `${rootId}\u0000${trimmed}`;
  if (highlightKey !== currentKey) {
    setHighlightKey(currentKey);
    setActiveIndex(0);
  }

  const panelRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const navigate = useNavigate();
  const listId = useId();

  useFocusTrap(panelRef);
  useEscapeKey(onClose);
  useBodyScrollLock('locked');

  useEffect(() => {
    if (!trimmed) return;
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      search(rootId, trimmed, controller.signal)
        .then((found) => setResult({ rootId, query: trimmed, hits: found, error: null }))
        .catch((cause: unknown) => {
          if (controller.signal.aborted) return;
          setResult({
            rootId,
            query: trimmed,
            hits: [],
            error: cause instanceof Error ? cause : new Error(String(cause)),
          });
        });
    }, DEBOUNCE_MS);

    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [trimmed, rootId]);

  // Keep the highlighted row inside the scroll viewport as arrows move it.
  useEffect(() => {
    listRef.current?.querySelectorAll('li')[activeIndex]?.scrollIntoView({ block: 'nearest' });
  }, [activeIndex]);

  const open = useCallback(
    (hit: SearchHit) => {
      onClose();
      void navigate(docRoute(rootId, hit.path));
    },
    [navigate, onClose, rootId],
  );

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (hits.length === 0) return;
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setActiveIndex((index) => (index + 1) % hits.length);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setActiveIndex((index) => (index - 1 + hits.length) % hits.length);
    } else if (event.key === 'Home') {
      event.preventDefault();
      setActiveIndex(0);
    } else if (event.key === 'End') {
      event.preventDefault();
      setActiveIndex(hits.length - 1);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      const hit = hits[activeIndex];
      if (hit) open(hit);
    }
  };

  const showEmpty = trimmed.length > 0 && !loading && !error && hits.length === 0;

  return createPortal(
    <div
      className="palette"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="palette__panel" role="dialog" aria-modal="true" aria-label={`Search ${rootName}`} ref={panelRef}>
        <input
          className="palette__input"
          type="search"
          role="combobox"
          aria-expanded={hits.length > 0}
          aria-controls={listId}
          aria-activedescendant={hits.length > 0 ? `${listId}-option-${activeIndex}` : undefined}
          aria-label={`Search ${rootName}`}
          placeholder={`Search ${rootName}…`}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={onKeyDown}
          autoComplete="off"
          spellCheck={false}
          data-autofocus
        />

        {loading ? (
          <div className="state" style={{ minHeight: 96 }}>
            <Spinner label="Searching" />
          </div>
        ) : null}

        {error ? (
          <div className="state" style={{ minHeight: 96 }} role="alert">
            <p className="state__title">{describeError(error).title}</p>
            <p className="state__detail">{describeError(error).detail}</p>
          </div>
        ) : null}

        {showEmpty ? (
          <div className="state" style={{ minHeight: 96 }}>
            <p className="state__detail">No matches for “{trimmed}”.</p>
          </div>
        ) : null}

        {!trimmed && !loading ? (
          <div className="state" style={{ minHeight: 96 }}>
            <p className="state__detail">Type to search titles and contents of {rootName}.</p>
          </div>
        ) : null}

        {/*
          A screen-reader user gets no feedback that a query returned nothing, or how many
          results arrived, unless it is announced: the listbox updating is silent until
          they navigate into it. WCAG 2.2 SC 4.1.3.
        */}
        <p className="sr-only" role="status" aria-live="polite">
          {loading
            ? 'Searching…'
            : !trimmed
              ? ''
              : hits.length === 0
                ? `No matches for ${trimmed}`
                : `${hits.length} ${hits.length === 1 ? 'result' : 'results'} for ${trimmed}`}
        </p>

        <ul className="palette__results" role="listbox" id={listId} aria-label="Search results" ref={listRef}>
          {hits.map((hit, index) => (
            <li
              key={`${hit.path}-${index}`}
              id={`${listId}-option-${index}`}
              role="option"
              aria-selected={index === activeIndex}
            >
              <button
                type="button"
                tabIndex={-1}
                className={`palette__hit${index === activeIndex ? ' palette__hit--active' : ''}`}
                onMouseEnter={() => setActiveIndex(index)}
                onClick={() => open(hit)}
              >
                <span className="kind-icon" aria-hidden="true">
                  {kindIcon(hit.kind)}
                </span>
                <span className="palette__hit-main">
                  <span className="palette__hit-title">{hit.title || hit.path}</span>
                  <span className="palette__hit-path">{hit.path}</span>
                  <Snippet snippet={hit.snippet} />
                </span>
              </button>
            </li>
          ))}
        </ul>

        <div className="palette__foot">
          <span>
            <kbd>↑</kbd> <kbd>↓</kbd> to move
          </span>
          <span>
            <kbd>Enter</kbd> to open
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
