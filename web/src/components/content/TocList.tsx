import type { MouseEvent } from 'react';
import type { Heading } from '../../api/types';

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

export interface TocListProps {
  headings: Heading[];
  shallowest: number;
  /** The section the reader is in, marked so a long list has an anchor. */
  currentSlug?: string | null;
  /** Runs after a jump, so a layer holding the list can close itself. */
  onNavigate?: () => void;
}

const INDENT_PER_LEVEL_PX = 12;

export function TocList({ headings, shallowest, currentSlug = null, onNavigate }: TocListProps) {
  return (
    <ul className="toc__list">
      {headings.map((heading) => (
        <li key={`${heading.slug}-${heading.depth}-${heading.text}`}>
          <a
            className={`toc__link${heading.slug === currentSlug ? ' toc__link--current' : ''}`}
            href={`#${heading.slug}`}
            aria-current={heading.slug === currentSlug ? 'location' : undefined}
            style={{ paddingLeft: `${(heading.depth - shallowest) * INDENT_PER_LEVEL_PX}px` }}
            onClick={(event) => {
              scrollToHeading(event, heading.slug);
              onNavigate?.();
            }}
          >
            {heading.text}
          </a>
        </li>
      ))}
    </ul>
  );
}
