import { useSyncExternalStore } from 'react';

/** Subscribes to a media query without an effect, so SSR-free first paint is correct. */
export function useMediaQuery(query: string): boolean {
  const subscribe = (onChange: () => void) => {
    const list = window.matchMedia(query);
    list.addEventListener('change', onChange);
    return () => list.removeEventListener('change', onChange);
  };
  return useSyncExternalStore(
    subscribe,
    () => window.matchMedia(query).matches,
    () => false,
  );
}

/** Below the phone breakpoint: the table of contents becomes a sheet behind a floating button. */
export const PHONE_QUERY = '(max-width: 899px)';

/**
 * Wide enough for the reading column to carry a margin on each side: the contents in the
 * left one, tags and links in the right. Must agree with the `.doc--reading` grid in app.css.
 */
export const WIDE_QUERY = '(min-width: 1100px)';

export function useIsWide(): boolean {
  return useMediaQuery(WIDE_QUERY);
}
