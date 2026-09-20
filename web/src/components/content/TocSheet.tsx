import { useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { Heading } from '../../api/types';
import { useBodyScrollLock } from '../../hooks/useBodyScrollLock';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useInertBackground } from '../../hooks/useInertBackground';
import { Button } from '../ui/Button';
import { TocList } from './TocList';

interface TocSheetButtonProps {
  headings: Heading[];
  shallowest: number;
}

/**
 * How far below the top of the scrolling pane a heading may sit and still count as the
 * current section. Wider than the headings' scroll-margin-top, so the heading a jump has
 * just landed on is the one marked.
 */
const CURRENT_SECTION_MARGIN_PX = 96;

/** Where the pane that scrolls the document starts; the viewport's top when it is the window. */
function scrollerTop(element: Element): number {
  for (let node = element.parentElement; node; node = node.parentElement) {
    const overflow = getComputedStyle(node).overflowY;
    if (overflow === 'auto' || overflow === 'scroll') return node.getBoundingClientRect().top;
  }
  return 0;
}

/**
 * The last heading at or above the reading line is the section being read; before the
 * reader has passed any heading, the first one is.
 */
function currentSectionSlug(headings: Heading[]): string | null {
  let current: string | null = headings[0]?.slug ?? null;
  let readingLine: number | null = null;
  for (const heading of headings) {
    const element = document.getElementById(heading.slug);
    if (!element) continue;
    readingLine ??= scrollerTop(element) + CURRENT_SECTION_MARGIN_PX;
    if (element.getBoundingClientRect().top > readingLine) break;
    current = heading.slug;
  }
  return current;
}

interface TocSheetProps extends TocSheetButtonProps {
  currentSlug: string | null;
  onClose: () => void;
}

function TocSheet({ headings, shallowest, currentSlug, onClose }: TocSheetProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const titleId = useId();

  // Same order as Modal: the background is focusable again before the trap hands focus back.
  useInertBackground();
  useFocusTrap(panelRef);
  useEscapeKey(onClose);
  useBodyScrollLock('locked');

  return createPortal(
    <div
      className="toc-sheet-scrim"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="toc-sheet" role="dialog" aria-modal="true" aria-labelledby={titleId} ref={panelRef}>
        <div className="toc-sheet__handle" aria-hidden="true" />
        <div className="toc-sheet__head">
          <h2 className="toc-sheet__title" id={titleId}>
            Contents
          </h2>
          <Button variant="icon" onClick={onClose} aria-label="Close contents">
            ✕
          </Button>
        </div>
        <nav className="toc toc--sheet" aria-label="Table of contents">
          <TocList headings={headings} shallowest={shallowest} currentSlug={currentSlug} onNavigate={onClose} />
        </nav>
      </div>
    </div>,
    document.body,
  );
}

/**
 * The phone's table of contents: a floating button that opens a bottom sheet, reachable
 * from anywhere in the document rather than only from its top.
 */
export function TocSheetButton({ headings, shallowest }: TocSheetButtonProps) {
  const [open, setOpen] = useState(false);
  const [currentSlug, setCurrentSlug] = useState<string | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef(false);

  const show = () => {
    setCurrentSlug(currentSectionSlug(headings));
    setOpen(true);
  };
  const hide = () => {
    returnFocus.current = true;
    setOpen(false);
  };

  // Focus comes back only once the sheet is gone: while it is open the page behind it is
  // inert and ignores focus(), and a tap never focused the button in the first place, so
  // the trap's own restore has nowhere to send it.
  useEffect(() => {
    if (open || !returnFocus.current) return;
    returnFocus.current = false;
    buttonRef.current?.focus({ preventScroll: true });
  }, [open]);

  return (
    <>
      <Button
        ref={buttonRef}
        variant="primary"
        className="toc-fab"
        onClick={show}
        aria-label="Table of contents"
        aria-haspopup="dialog"
        aria-expanded={open}
      >
        Contents
      </Button>
      {open ? <TocSheet headings={headings} shallowest={shallowest} currentSlug={currentSlug} onClose={hide} /> : null}
    </>
  );
}
