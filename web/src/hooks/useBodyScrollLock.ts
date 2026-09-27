import { useEffect } from 'react';

export type BodyScroll = 'locked' | 'scrollable';

/**
 * How many layers want the page held still, and what to put back when the last one goes.
 *
 * Counted across all callers rather than snapshotted per caller, because the callers
 * overlap: a drawer and a dialog opened from inside it are both holding the lock, and each
 * restoring its own remembered value meant whichever unmounted second wrote `hidden` back.
 * That leak is invisible on a document route, which scrolls an inner pane, and total on the
 * home route, which scrolls the document itself — a page the reader cannot move, with
 * nothing on screen to explain why.
 */
let holders = 0;
let restoreTo = '';

/** Stops the page behind a modal or drawer from scrolling on touch devices. */
export function useBodyScrollLock(scroll: BodyScroll): void {
  useEffect(() => {
    if (scroll === 'scrollable') return;
    if (holders === 0) {
      restoreTo = document.body.style.overflow;
      document.body.style.overflow = 'hidden';
    }
    holders += 1;
    return () => {
      holders -= 1;
      if (holders === 0) {
        document.body.style.overflow = restoreTo;
      }
    };
  }, [scroll]);
}
