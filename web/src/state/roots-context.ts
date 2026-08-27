import { createContext, useContext } from 'react';
import type { RootInfo } from '../api/types';

export interface RootsContextValue {
  roots: RootInfo[];
  loading: boolean;
  error: Error | null;
  reload: () => void;
}

export const RootsContext = createContext<RootsContextValue | null>(null);

export function useRoots(): RootsContextValue {
  const value = useContext(RootsContext);
  if (!value) throw new Error('useRoots must be used inside <RootsProvider>');
  return value;
}
