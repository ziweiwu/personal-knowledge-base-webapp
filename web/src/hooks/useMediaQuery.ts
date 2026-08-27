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

export const DESKTOP_QUERY = '(min-width: 900px)';

export function useIsDesktop(): boolean {
  return useMediaQuery(DESKTOP_QUERY);
}
