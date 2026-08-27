import { useCallback, useMemo, type ReactNode } from 'react';
import { fetchRoots } from '../api/client';
import { useAsyncResource } from '../hooks/useAsyncResource';
import { RootsContext } from './roots-context';

export function RootsProvider({ children }: { children: ReactNode }) {
  const load = useCallback((signal: AbortSignal) => fetchRoots(signal), []);
  const resource = useAsyncResource(load);

  const value = useMemo(
    () => ({
      roots: resource.data ?? [],
      loading: resource.loading,
      error: resource.error,
      reload: resource.reload,
    }),
    [resource.data, resource.loading, resource.error, resource.reload],
  );

  return <RootsContext value={value}>{children}</RootsContext>;
}
