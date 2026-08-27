import { useEffect } from 'react';

export type BodyScroll = 'locked' | 'scrollable';

/** Stops the page behind a modal or drawer from scrolling on touch devices. */
export function useBodyScrollLock(scroll: BodyScroll): void {
  useEffect(() => {
    if (scroll === 'scrollable') return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    return () => {
      document.body.style.overflow = previous;
    };
  }, [scroll]);
}
