import { useEffect, useState, type RefObject } from 'react';

/** Scroll travel below which a change of direction is ignored, so a wobble never flickers the strip. */
const DIRECTION_THRESHOLD_PX = 8;

/** Within this distance of the top the strip is always shown. */
const TOP_ZONE_PX = 24;

/** A pane with less scroll range than this never hides the strip: it would flap for nothing. */
const MIN_SCROLL_RANGE_PX = 240;

/**
 * Whether the top strip should be out of the way: the pane is scrolling down, past the
 * top, on a page long enough to be worth the room. Scrolling up, or reaching the top,
 * brings it back. A route change resets the pane's offset, which fires a scroll event
 * of its own, so a new page always starts with the strip shown.
 */
export function useHiddenOnScroll(containerRef: RefObject<HTMLElement | null>): boolean {
  const [hidden, setHidden] = useState(false);

  useEffect(() => {
    const pane = containerRef.current;
    if (!pane) return;
    let lastTop = pane.scrollTop;
    const onScroll = () => {
      const top = pane.scrollTop;
      const range = pane.scrollHeight - pane.clientHeight;
      if (top <= TOP_ZONE_PX || range < MIN_SCROLL_RANGE_PX) {
        setHidden(false);
        lastTop = top;
        return;
      }
      const delta = top - lastTop;
      if (Math.abs(delta) < DIRECTION_THRESHOLD_PX) return;
      setHidden(delta > 0);
      lastTop = top;
    };
    pane.addEventListener('scroll', onScroll, { passive: true });
    return () => pane.removeEventListener('scroll', onScroll);
  }, [containerRef]);

  return hidden;
}
