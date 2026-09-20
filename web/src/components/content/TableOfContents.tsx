import { useState } from 'react';
import type { Heading } from '../../api/types';
import { PHONE_QUERY, useMediaQuery } from '../../hooks/useMediaQuery';
import { TocList } from './TocList';
import { TocSheetButton } from './TocSheet';

interface TableOfContentsProps {
  headings: Heading[];
  variant: 'rail' | 'inline';
}

/** Levels shown by default, counted from the document's shallowest heading. */
const DEFAULT_VISIBLE_LEVELS = 3;

export function TableOfContents({ headings, variant }: TableOfContentsProps) {
  const [showAll, setShowAll] = useState(false);
  const phone = useMediaQuery(PHONE_QUERY);
  if (headings.length < 2) return null;
  const shallowest = Math.min(...headings.map((heading) => heading.depth));
  // A phone gets a floating button and a bottom sheet instead of a block at the top,
  // so jumping sections mid-read never means scrolling back up first.
  if (variant === 'inline' && phone) return <TocSheetButton headings={headings} shallowest={shallowest} />;
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
