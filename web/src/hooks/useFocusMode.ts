import { useCallback, useEffect, useMemo, useState } from 'react';

export interface FocusMode {
  focused: boolean;
  enter: () => void;
  exit: () => void;
  toggle: () => void;
}

/**
 * Native fullscreen where the browser offers it; the CSS mode everywhere else.
 *
 * iOS Safari implements `requestFullscreen` for `<video>` and nothing else, and a phone is
 * this app's primary target — so the browser API is the enhancement and the stylesheet is
 * what actually delivers the mode. A refused request must therefore leave focus mode on
 * rather than tear it down.
 */
async function requestNativeFullscreen(): Promise<void> {
  if (!document.documentElement.requestFullscreen || document.fullscreenElement) return;
  try {
    await document.documentElement.requestFullscreen({ navigationUI: 'hide' });
  } catch {
    /* unsupported, or refused outside a user gesture; the CSS mode still applies */
  }
}

async function leaveNativeFullscreen(): Promise<void> {
  if (!document.fullscreenElement || !document.exitFullscreen) return;
  try {
    await document.exitFullscreen();
  } catch {
    /* already gone */
  }
}

/**
 * Distraction-free reading: the app chrome goes away and the document is all that is left.
 *
 * Deliberately not persisted. Every other view preference in this app survives a restart,
 * but this one hides every navigation control, so restoring into it would leave the app
 * with a single small button as its only exit.
 */
export function useFocusMode(): FocusMode {
  const [focused, setFocused] = useState(false);

  const enter = useCallback(() => {
    setFocused(true);
    void requestNativeFullscreen();
  }, []);

  const exit = useCallback(() => {
    setFocused(false);
    void leaveNativeFullscreen();
  }, []);

  const toggle = useCallback(() => {
    if (focused) exit();
    else enter();
  }, [focused, enter, exit]);

  // Leaving fullscreen by the browser's own gesture — Escape, or the OS chrome — must not
  // strand the page stripped of its chrome while this hook still believes it is in focus
  // mode. Never fires where fullscreen is unsupported, which is exactly right.
  useEffect(() => {
    const onFullscreenChange = () => {
      if (!document.fullscreenElement) setFocused(false);
    };
    document.addEventListener('fullscreenchange', onFullscreenChange);
    return () => document.removeEventListener('fullscreenchange', onFullscreenChange);
  }, []);

  // Memoised so the identity changes only when the mode does. Callers subscribe key
  // listeners against it; a fresh object every render would rebind them every render.
  return useMemo(() => ({ focused, enter, exit, toggle }), [focused, enter, exit, toggle]);
}
