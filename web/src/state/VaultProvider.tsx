import { useCallback, useMemo, useState, type ReactNode } from 'react';
import { fetchTree } from '../api/client';
import { eventTouches } from '../api/events';
import type { ChangeEvent } from '../api/types';
import { useAsyncResource } from '../hooks/useAsyncResource';
import { useChangeEvents } from '../hooks/useChangeEvents';
import { useRoots } from './roots-context';
import { VaultContext } from './vault-context';

interface VaultProviderProps {
  rootId: string;
  children: ReactNode;
}

export function VaultProvider({ rootId, children }: VaultProviderProps) {
  const { roots } = useRoots();
  const root = useMemo(() => roots.find((candidate) => candidate.id === rootId) ?? null, [roots, rootId]);

  const load = useCallback((signal: AbortSignal) => fetchTree(rootId, signal), [rootId]);
  const tree = useAsyncResource(load);

  const { reload: reloadTree } = tree;
  const onChange = useCallback(
    (event: ChangeEvent) => {
      // Any change inside this root can add or remove a node.
      if (eventTouches(event, rootId, '')) reloadTree();
    },
    [rootId, reloadTree],
  );
  useChangeEvents(onChange);

  const [localChangeToken, setLocalChangeToken] = useState(0);
  const notifyLocalChange = useCallback(() => setLocalChangeToken((current) => current + 1), []);

  const value = useMemo(
    () => ({
      rootId,
      root,
      canEdit: root ? !root.readOnly : true,
      tree: tree.data,
      treeLoading: tree.loading,
      treeError: tree.error,
      reloadTree,
      localChangeToken,
      notifyLocalChange,
    }),
    [rootId, root, tree.data, tree.loading, tree.error, reloadTree, localChangeToken, notifyLocalChange],
  );

  return <VaultContext value={value}>{children}</VaultContext>;
}
