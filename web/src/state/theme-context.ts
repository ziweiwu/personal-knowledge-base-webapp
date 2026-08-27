import { createContext, useContext } from 'react';

export type ThemeChoice = 'light' | 'dark' | 'system';

export const THEME_STORAGE_KEY = 'kbview.theme';

export interface ThemeContextValue {
  choice: ThemeChoice;
  /** What is actually on screen once `system` has been resolved. */
  resolved: 'light' | 'dark';
  setChoice: (choice: ThemeChoice) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);

export function useTheme(): ThemeContextValue {
  const value = useContext(ThemeContext);
  if (!value) throw new Error('useTheme must be used inside <ThemeProvider>');
  return value;
}
