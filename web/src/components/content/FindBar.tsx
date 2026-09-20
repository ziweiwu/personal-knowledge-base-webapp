import { useEffect, useRef, useState, type KeyboardEvent, type RefObject } from 'react';
import { clearHighlights, focusMatch, highlightMatches } from '../../lib/findInPage';
import { Button } from '../ui/Button';
import { wrapIndex } from '../../lib/wrapIndex';
import { Icon } from '../ui/Icon';

interface FindBarProps {
  /** The prose container to search. */
  containerRef: RefObject<HTMLElement | null>;
  /** The markup the container currently holds; a change means the marks are gone too. */
  html: string;
  onClose: () => void;
}

export function FindBar({ containerRef, html, onClose }: FindBarProps) {
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const [count, setCount] = useState(0);
  const marks = useRef<HTMLElement[]>([]);

  // Re-run when the markup changes: HtmlContent has replaced the container's
  // contents by then, taking the old marks with it.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    clearHighlights(container);
    marks.current = highlightMatches(container, query);
    setCount(marks.current.length);
    setIndex(0);
    focusMatch(marks.current, 0);
    return () => clearHighlights(container);
  }, [containerRef, html, query]);

  useEffect(() => focusMatch(marks.current, index), [index]);

  const step = (delta: number) => setIndex((current) => wrapIndex(current + delta, count));

  const close = () => {
    onClose();
    // Hand focus back to the prose so the next keystroke lands in the document.
    const container = containerRef.current;
    if (!container) return;
    container.setAttribute('tabindex', '-1');
    container.focus({ preventScroll: true });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      step(event.shiftKey ? -1 : 1);
    } else if (event.key === 'Escape') {
      // Ours, not focus mode's: Escape in the bar closes the bar and nothing else.
      event.preventDefault();
      event.stopPropagation();
      close();
    }
  };

  const status = !query ? '' : count === 0 ? 'No matches' : `${index + 1} of ${count}`;

  return (
    <div className="find-bar" role="search" aria-label="Find in document">
      <input
        className="find-bar__input"
        type="search"
        aria-label="Find in document"
        placeholder="Find in document…"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={onKeyDown}
        autoFocus
      />
      <span className="find-bar__count" aria-live="polite">
        {status}
      </span>
      <Button variant="ghost" onClick={() => step(-1)} disabled={count === 0} aria-label="Previous match">
        <Icon name="arrow-up" />
      </Button>
      <Button variant="ghost" onClick={() => step(1)} disabled={count === 0} aria-label="Next match">
        <Icon name="arrow-down" />
      </Button>
      <Button variant="ghost" onClick={close} aria-label="Close find">
        <Icon name="close" />
      </Button>
    </div>
  );
}
