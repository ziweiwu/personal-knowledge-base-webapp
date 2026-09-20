import { useEffect, useState, type RefObject } from 'react';

/** How far `container` has been scrolled, 0 when there is nowhere to scroll to. */
function progressOf(container: HTMLElement): number {
  const range = container.scrollHeight - container.clientHeight;
  if (range <= 0) return 0;
  return Math.min(1, Math.max(0, container.scrollTop / range));
}

/** Keeps `sizes` pointed at the container's first child, which every navigation replaces. */
function firstChildWatcher(container: HTMLElement, sizes: ResizeObserver): { refresh: () => void } {
  let watched: Element | null = null;
  const refresh = () => {
    const content = container.firstElementChild;
    if (content === watched) return;
    if (watched) sizes.unobserve(watched);
    watched = content;
    if (content) sizes.observe(content);
  };
  refresh();
  return { refresh };
}

/**
 * Reports the container's progress at most once per animation frame, however often the
 * pane scrolls or its content grows; returns the teardown.
 */
function observeProgress(container: HTMLElement, onChange: (ratio: number) => void): () => void {
  let frame = 0;
  const measure = () => {
    frame = 0;
    onChange(progressOf(container));
  };
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(measure);
  };
  const sizes = new ResizeObserver(schedule);
  sizes.observe(container);
  const content = firstChildWatcher(container, sizes);
  const children = new MutationObserver(() => {
    content.refresh();
    schedule();
  });
  children.observe(container, { childList: true });
  container.addEventListener('scroll', schedule, { passive: true });
  schedule();
  return () => {
    cancelAnimationFrame(frame);
    container.removeEventListener('scroll', schedule);
    children.disconnect();
    sizes.disconnect();
  };
}

/**
 * The fraction of `containerRef` that has scrolled past, as a number from 0 to 1.
 *
 * Content is fetched after the pane mounts and images arrive later still, so the pane's
 * first child is watched for size changes and swapped whenever navigation replaces it.
 */
export function useReadingProgress(containerRef: RefObject<HTMLElement | null>): number {
  const [ratio, setRatio] = useState(0);
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    return observeProgress(container, setRatio);
  }, [containerRef]);
  return ratio;
}
