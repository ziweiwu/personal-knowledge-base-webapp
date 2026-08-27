import { useLayoutEffect, type RefObject } from 'react';
import { useLocation, useNavigationType } from 'react-router-dom';

/** Where each history entry was last left, keyed by its router location key. */
const offsets = new Map<string, number>();

/**
 * How long a back/forward navigation keeps reaching for the recorded offset.
 * The pane's content is fetched, so the target is normally out of reach for the
 * first few frames; once it is reachable the loop stops immediately.
 */
const RESTORE_BUDGET_MS = 2000;

/** A deliberate scroll by the user outranks a restore that is still catching up. */
const ABORT_EVENTS = ['wheel', 'touchstart', 'pointerdown', 'keydown'] as const;

/**
 * Gives `containerRef` browser-like scroll behaviour across SPA navigation: a
 * new page starts at the top, but back and forward return to where the entry
 * was left.
 *
 * The app scrolls an inner pane rather than the window, so the browser's own
 * restoration has nothing to act on and is switched off to keep the two from
 * disagreeing.
 */
export function useScrollRestoration(containerRef: RefObject<HTMLElement | null>): void {
  const { key } = useLocation();
  const navigationType = useNavigationType();

  useLayoutEffect(() => {
    const previous = history.scrollRestoration;
    history.scrollRestoration = 'manual';
    return () => {
      history.scrollRestoration = previous;
    };
  }, []);

  // Recorded continuously rather than on teardown: by the time an entry is torn
  // down the pane has already been clamped against the incoming page's height,
  // so reading the offset then would store the clamped value, not the real one.
  useLayoutEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const record = () => offsets.set(key, container.scrollTop);
    container.addEventListener('scroll', record, { passive: true });
    return () => container.removeEventListener('scroll', record);
  }, [containerRef, key]);

  useLayoutEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    if (navigationType !== 'POP') {
      container.scrollTop = 0;
      offsets.set(key, 0);
      return;
    }

    const target = offsets.get(key) ?? 0;
    if (target <= 0) return;
    return restoreOffset(container, target);
  }, [containerRef, key, navigationType]);
}

/**
 * Drive `container` to `target`, retrying each frame until the offset is reachable or the
 * budget runs out. Returns the teardown, which also cancels a restore still in progress.
 */
function restoreOffset(container: HTMLElement, target: number): () => void {
  let frame = 0;
  const deadline = performance.now() + RESTORE_BUDGET_MS;

  const stop = () => {
    cancelAnimationFrame(frame);
    for (const name of ABORT_EVENTS) container.removeEventListener(name, stop);
  };

  const attempt = () => {
    container.scrollTop = target;
    const reached = Math.abs(container.scrollTop - target) <= 1;
    if (reached || performance.now() > deadline) stop();
    else frame = requestAnimationFrame(attempt);
  };

  for (const name of ABORT_EVENTS) container.addEventListener(name, stop, { passive: true });
  attempt();
  return stop;
}
