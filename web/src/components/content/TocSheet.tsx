import { useEffect, useRef, useState } from 'react';
import type { Heading } from '../../api/types';
import { Button } from '../ui/Button';
import { DialogShell } from '../ui/DialogShell';
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
  return (
    <DialogShell
      title="Contents"
      closeLabel="Close contents"
      onClose={onClose}
      onBackdrop={onClose}
      block="toc-sheet"
      handle={<div className="toc-sheet__handle" aria-hidden="true" />}
    >
      <nav className="toc toc--sheet" aria-label="Table of contents">
        <TocList headings={headings} shallowest={shallowest} currentSlug={currentSlug} onNavigate={onClose} />
      </nav>
    </DialogShell>
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
