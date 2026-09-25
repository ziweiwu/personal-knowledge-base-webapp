import { useState } from 'react';
import type { Heading } from '../../api/types';
import { PHONE_QUERY, useMediaQuery } from '../../hooks/useMediaQuery';
import { hasTableOfContents } from '../../lib/headings';
import { TocList } from './TocList';
import { TocSheetButton } from './TocSheet';

interface TableOfContentsProps {
  headings: Heading[];
  variant: 'rail' | 'inline';
}

/** Levels shown by default, counted from the document's shallowest heading. */
const DEFAULT_VISIBLE_LEVELS = 3;

interface TocBlockProps {
  headings: Heading[];
  shallowest: number;
  variant: 'rail' | 'inline';
}

/** The list with a "show all levels" toggle, as the desktop rail or the inline block. */
function TocBlock({ headings, shallowest, variant }: TocBlockProps) {
  const [showAll, setShowAll] = useState(false);
  const deepestShown = shallowest + DEFAULT_VISIBLE_LEVELS - 1;
  const hasDeeper = headings.some((heading) => heading.depth > deepestShown);
  const shown = showAll ? headings : headings.filter((heading) => heading.depth <= deepestShown);
  return (
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
}

/**
 * A phone gets a floating button and a bottom sheet instead of a block at the top, so
 * jumping sections mid-read never means scrolling back up first. Wider than a phone, the
 * block collapses under a summary line.
 */
function InlineTableOfContents({ headings, shallowest }: Omit<TocBlockProps, 'variant'>) {
  const phone = useMediaQuery(PHONE_QUERY);
  if (phone) return <TocSheetButton headings={headings} shallowest={shallowest} />;
  return (
    <details className="toc-mobile">
      <summary>On this page ({headings.length})</summary>
      <TocBlock headings={headings} shallowest={shallowest} variant="inline" />
    </details>
  );
}

export function TableOfContents({ headings, variant }: TableOfContentsProps) {
  if (!hasTableOfContents(headings)) return null;
  const shallowest = Math.min(...headings.map((heading) => heading.depth));
  if (variant === 'rail') return <TocBlock headings={headings} shallowest={shallowest} variant="rail" />;
  return <InlineTableOfContents headings={headings} shallowest={shallowest} />;
}
