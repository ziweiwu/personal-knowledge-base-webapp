import { useState, type MouseEvent } from 'react';
import type { Heading } from '../../api/types';

interface TableOfContentsProps {
  headings: Heading[];
  variant: 'rail' | 'inline';
}

/** Levels shown by default, counted from the document's shallowest heading. */
const DEFAULT_VISIBLE_LEVELS = 3;

function scrollToHeading(event: MouseEvent<HTMLAnchorElement>, slug: string): void {
  const target = document.getElementById(slug);
  if (!target) return;
  event.preventDefault();
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  target.scrollIntoView({ behavior: reduceMotion ? 'auto' : 'smooth', block: 'start' });
  // Move focus too, so the keyboard caret follows the jump.
  target.setAttribute('tabindex', '-1');
  target.focus({ preventScroll: true });
  history.replaceState(null, '', `#${slug}`);
}

interface TocListProps {
  headings: Heading[];
  shallowest: number;
}

function TocList({ headings, shallowest }: TocListProps) {
  return (
    <ul className="toc__list">
      {headings.map((heading) => (
        <li key={`${heading.slug}-${heading.depth}-${heading.text}`}>
          <a
            className="toc__link"
            href={`#${heading.slug}`}
            style={{ paddingLeft: `${(heading.depth - shallowest) * 12}px` }}
            onClick={(event) => scrollToHeading(event, heading.slug)}
          >
            {heading.text}
          </a>
        </li>
      ))}
    </ul>
  );
}

export function TableOfContents({ headings, variant }: TableOfContentsProps) {
  const [showAll, setShowAll] = useState(false);
  if (headings.length < 2) return null;
  const shallowest = Math.min(...headings.map((heading) => heading.depth));
  const deepestShown = shallowest + DEFAULT_VISIBLE_LEVELS - 1;
  const hasDeeper = headings.some((heading) => heading.depth > deepestShown);
  const shown = showAll ? headings : headings.filter((heading) => heading.depth <= deepestShown);

  const list = (
    <nav className={`toc${variant === 'rail' ? ' toc--rail' : ''}`} aria-label="Table of contents">
      {variant === 'rail' ? <p className="toc__heading">On this page</p> : null}
      <TocList headings={shown} shallowest={shallowest} />
      {hasDeeper ? (
        <button type="button" className="toc__toggle" onClick={() => setShowAll((value) => !value)}>
          {showAll ? 'Fewer levels' : 'Show all levels'}
        </button>
      ) : null}
    </nav>
  );

  if (variant === 'rail') return list;

  return (
    <details className="toc-mobile">
      <summary>On this page ({headings.length})</summary>
      {list}
    </details>
  );
}
