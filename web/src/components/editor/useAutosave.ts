import { useEffect, useRef } from 'react';

export const AUTOSAVE_IDLE_SECONDS = 10;
const MS_PER_SECOND = 1000;
export const AUTOSAVE_IDLE_MS = AUTOSAVE_IDLE_SECONDS * MS_PER_SECOND;

interface AutosaveOptions {
  /**
   * The save to run once the buffer has been idle, or null while saving would be wrong:
   * nothing is dirty, a save is in flight, a conflict is open, the base is stale, or the
   * last attempt failed and no edit has happened since.
   */
  save: (() => void) | null;
  /** Grows with every edit; each change restarts the idle window. */
  editCount: number;
}

/**
 * Saves the buffer through the ordinary save path after a quiet spell, and flushes it at
 * once when the page is hidden or unloading — the case mobile Safari never reports through
 * `beforeunload`, so a backgrounded tab used to be the one way to lose text.
 */
export function useAutosave({ save, editCount }: AutosaveOptions): void {
  const latestSave = useRef(save);
  useEffect(() => {
    latestSave.current = save;
  }, [save]);

  const armed = save !== null;
  useEffect(() => {
    if (!armed) return;
    let fired = false;
    const fire = () => {
      if (fired) return;
      fired = true;
      window.clearTimeout(timer);
      latestSave.current?.();
    };
    const timer = window.setTimeout(fire, AUTOSAVE_IDLE_MS);
    const onVisibilityChange = () => {
      if (document.visibilityState === 'hidden') fire();
    };
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('pagehide', fire);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener('visibilitychange', onVisibilityChange);
      window.removeEventListener('pagehide', fire);
    };
  }, [armed, editCount]);
}
