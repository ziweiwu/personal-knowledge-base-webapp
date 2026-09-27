import { useEffect, useRef } from 'react';

/** One open layer's way out, so the stack can close the innermost one. */
interface EscapeLayer {
  close: () => void;
}

/**
 * Every layer currently open, innermost last.
 *
 * One stack and one listener, rather than a listener per layer. Sibling listeners on
 * `document` all fire for the same keypress, and `stopPropagation` does not change that —
 * it stops the event travelling onwards, not the other handlers already attached to the
 * same node. So a drawer and the dialog above it both saw Escape, and which one actually
 * closed came down to whose listener happened to still be attached when the event reached
 * it. A caller passing a fresh arrow each render made that worse: its listener was
 * removed and re-added while the event was being dispatched, and a listener re-added
 * mid-dispatch is skipped, so the layer on top silently missed the keypress that was
 * meant for it.
 *
 * Position in the stack is the order layers opened in, which is the order they must close
 * in. Nothing else in the app knows that order.
 */
const openLayers: EscapeLayer[] = [];

function closeInnermostLayer(event: KeyboardEvent) {
  if (event.key !== 'Escape') return;
  const innermost = openLayers[openLayers.length - 1];
  if (!innermost) return;
  event.stopPropagation();
  innermost.close();
}

/**
 * Escape closes this layer, once it is the innermost one open. Pass `null` while the layer
 * is closed and it takes no part in the stack.
 */
export function useEscapeKey(onEscape: (() => void) | null): void {
  const latest = useRef(onEscape);
  const open = onEscape !== null;

  // Kept current without re-entering the stack, so only opening and closing move a layer.
  useEffect(() => {
    latest.current = onEscape;
  }, [onEscape]);

  useEffect(() => {
    if (!open) return;
    // Read through the ref, so a caller whose callback identity changes every render
    // keeps its place instead of being torn down and pushed back on top.
    const layer: EscapeLayer = { close: () => latest.current?.() };
    openLayers.push(layer);
    if (openLayers.length === 1) {
      document.addEventListener('keydown', closeInnermostLayer);
    }
    return () => {
      openLayers.splice(openLayers.indexOf(layer), 1);
      if (openLayers.length === 0) {
        document.removeEventListener('keydown', closeInnermostLayer);
      }
    };
  }, [open]);
}
