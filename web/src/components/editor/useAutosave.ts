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
  /**
   * The save to run as the page is hidden or unloading, null under the same conditions.
   * It must issue a request that outlives the page; a plain one is cancelled with it.
   */
  flush: (() => void) | null;
  /** Grows with every edit; each change restarts the idle window. */
  editCount: number;
}

/**
 * Saves the buffer through the ordinary save path after a quiet spell, and flushes it at
 * once when the page is hidden or unloading — the case mobile Safari never reports through
 * `beforeunload`, so a backgrounded tab used to be the one way to lose text.
 */
export function useAutosave({ save, flush, editCount }: AutosaveOptions): void {
  const latest = useRef({ save, flush });
  useEffect(() => {
    latest.current = { save, flush };
  }, [save, flush]);

  const armed = save !== null;
  useEffect(() => {
    if (!armed) return;
    let fired = false;
    const once = (run: () => void) => {
      if (fired) return;
      fired = true;
      window.clearTimeout(timer);
      run();
    };
    const onIdle = () => once(() => latest.current.save?.());
    const onLeave = () => once(() => (latest.current.flush ?? latest.current.save)?.());
    const timer = window.setTimeout(onIdle, AUTOSAVE_IDLE_MS);
    const onVisibilityChange = () => {
      if (document.visibilityState === 'hidden') onLeave();
    };
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('pagehide', onLeave);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener('visibilitychange', onVisibilityChange);
      window.removeEventListener('pagehide', onLeave);
    };
  }, [armed, editCount]);
}
