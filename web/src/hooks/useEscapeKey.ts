import { useEffect } from 'react';

/**
 * Escape closes the topmost layer. Bound at the document so it works from
 * anywhere inside it; pass `null` while the layer is closed and nothing binds.
 */
export function useEscapeKey(onEscape: (() => void) | null): void {
  useEffect(() => {
    if (!onEscape) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        onEscape();
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [onEscape]);
}
