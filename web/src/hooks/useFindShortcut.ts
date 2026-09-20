import { useEffect } from 'react';

/** Whether a key event came from somewhere the user is typing. */
function isTypingTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  return element.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(element.tagName);
}

/**
 * Cmd/Ctrl+F opens the in-page find while a document's prose is showing. The
 * browser's own find is left alone whenever ours would be useless: no prose,
 * the editor open, or the keystroke aimed at another field. Pass null while
 * there is nothing to search and the shortcut stays unbound.
 */
export function useFindShortcut(open: (() => void) | null): void {
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== 'f' || event.altKey) return;
      const target = event.target as HTMLElement | null;
      if (isTypingTarget(target) && !target?.closest('.find-bar')) return;
      event.preventDefault();
      open();
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [open]);
}
