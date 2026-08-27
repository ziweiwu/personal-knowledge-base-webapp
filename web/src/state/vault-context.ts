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
  /**
   * Bumped after a mutation this client made. The event stream deliberately
   * drops our own echo, so nothing else tells the open folder or document that
   * its contents just changed underneath it.
   */
  localChangeToken: number;
  notifyLocalChange: () => void;
}

export const VaultContext = createContext<VaultContextValue | null>(null);

export function useVault(): VaultContextValue {
  const value = useContext(VaultContext);
  if (!value) throw new Error('useVault must be used inside <VaultProvider>');
  return value;
}
