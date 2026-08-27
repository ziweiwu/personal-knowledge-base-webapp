import { useEffect, type RefObject } from 'react';

const FOCUSABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

function focusable(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (element) => element.offsetParent !== null || element === document.activeElement,
  );
}

/**
 * Keeps Tab inside `containerRef` for as long as the hook is mounted, moves
 * focus in on mount and restores it to the previously focused element on
 * unmount. Callers mount it only while the layer they own is open.
 */
export function useFocusTrap(containerRef: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const previouslyFocused = document.activeElement as HTMLElement | null;
    const initial = container.querySelector<HTMLElement>('[data-autofocus]') ?? focusable(container)[0] ?? container;
    initial.focus({ preventScroll: true });

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Tab') return;
      const items = focusable(container);
      if (items.length === 0) {
        event.preventDefault();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const current = document.activeElement;
      if (event.shiftKey && (current === first || !container.contains(current))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && current === last) {
        event.preventDefault();
        first.focus();
      }
    };

    container.addEventListener('keydown', onKeyDown);
    return () => {
      container.removeEventListener('keydown', onKeyDown);
      previouslyFocused?.focus?.({ preventScroll: true });
    };
  }, [containerRef]);
}
