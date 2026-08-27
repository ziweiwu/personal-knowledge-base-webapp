import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { useMediaQuery } from '../hooks/useMediaQuery';
import { THEME_STORAGE_KEY, ThemeContext, type ThemeChoice } from './theme-context';

function readStoredChoice(): ThemeChoice {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    if (stored === 'light' || stored === 'dark' || stored === 'system') return stored;
  } catch {
    // Storage can be unavailable (Safari private browsing); the default applies.
  }
  return 'system';
}

function persistChoice(choice: ThemeChoice): void {
  try {
    if (choice === 'system') localStorage.removeItem(THEME_STORAGE_KEY);
    else localStorage.setItem(THEME_STORAGE_KEY, choice);
  } catch {
    // A theme that does not survive a reload beats a crash.
  }
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [choice, setChoiceState] = useState<ThemeChoice>(readStoredChoice);
  const prefersDark = useMediaQuery('(prefers-color-scheme: dark)');
  const resolved = choice === 'system' ? (prefersDark ? 'dark' : 'light') : choice;

  useEffect(() => {
    const root = document.documentElement;
    if (choice === 'system') delete root.dataset.theme;
    else root.dataset.theme = choice;
  }, [choice]);

  useEffect(() => {
    const meta = document.querySelector('meta[name="theme-color"]');
    if (!meta) return;
    // Read the resolved token so the browser chrome matches the page.
    const background = getComputedStyle(document.documentElement).getPropertyValue('--bg').trim();
    if (background) meta.setAttribute('content', background);
  }, [resolved]);

  const setChoice = useCallback((next: ThemeChoice) => {
    setChoiceState(next);
    persistChoice(next);
  }, []);

  const value = useMemo(() => ({ choice, resolved, setChoice }), [choice, resolved, setChoice]);

  return <ThemeContext value={value}>{children}</ThemeContext>;
}
