import { createContext, useContext } from 'react';
import type { RootInfo, TreeNode } from '../api/types';

export interface VaultContextValue {
  rootId: string;
  root: RootInfo | null;
  /** False when the root is read-only, hiding every mutating affordance. */
  canEdit: boolean;
  tree: TreeNode[] | null;
  treeLoading: boolean;
  treeError: Error | null;
  reloadTree: () => void;
}

export const VaultContext = createContext<VaultContextValue | null>(null);

export function useVault(): VaultContextValue {
  const value = useContext(VaultContext);
  if (!value) throw new Error('useVault must be used inside <VaultProvider>');
  return value;
}
