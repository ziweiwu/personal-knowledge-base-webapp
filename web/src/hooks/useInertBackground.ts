import { useEffect } from 'react';

/**
 * How many layers currently want the page behind them inert. Stacked dialogs
 * must not un-inert the background when only the topmost one closes, so the
 * count lives outside React and only the last release clears the attribute.
 */
let holders = 0;

/**
 * Marks the app root inert while a layer rendered *outside* it is open.
 *
 * `Modal` portals into `document.body`, so `#root` holds nothing the dialog
 * needs: making it inert removes the whole page from the accessibility tree and
 * stops a screen-reader user browsing behind the dialog by heading or arrow key.
 *
 * Call this *before* any hook that moves focus. Effect cleanups run in
 * declaration order, so releasing the attribute first leaves the background
 * focusable again by the time a focus trap restores focus into it.
 */
export function useInertBackground(): void {
  useEffect(() => {
    const root = document.getElementById('root');
    if (!root) return;
    holders += 1;
    root.inert = true;
    return () => {
      holders -= 1;
      if (holders === 0) root.inert = false;
    };
  }, []);
}
