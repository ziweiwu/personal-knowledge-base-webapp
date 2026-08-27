import type { MouseEvent } from 'react';
import type { Heading } from '../../api/types';

interface TableOfContentsProps {
  headings: Heading[];
  variant: 'rail' | 'inline';
}

function scrollToHeading(event: MouseEvent<HTMLAnchorElement>, slug: string): void {
  const target = document.getElementById(slug);
  if (!target) return;
  event.preventDefault();
  target.scrollIntoView({ behavior: 'smooth', block: 'start' });
  // Move focus too, so the keyboard caret follows the jump.
  target.setAttribute('tabindex', '-1');
  target.focus({ preventScroll: true });
  history.replaceState(null, '', `#${slug}`);
}

export function TableOfContents({ headings, variant }: TableOfContentsProps) {
  if (headings.length < 2) return null;
  const shallowest = Math.min(...headings.map((heading) => heading.depth));

  const list = (
    <nav className={`toc${variant === 'rail' ? ' toc--rail' : ''}`} aria-label="Table of contents">
      {variant === 'rail' ? <p className="toc__heading">On this page</p> : null}
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
