import { useCallback, useEffect, useState, type Dispatch, type SetStateAction } from 'react';

type Loader<T> = (signal: AbortSignal) => Promise<T>;

interface Settled<T> {
  /** Which loader produced this result, so a stale one is never shown. */
  loader: Loader<T>;
  token: number;
  data: T | null;
  error: Error | null;
}

export interface AsyncResource<T> {
  data: T | null;
  error: Error | null;
  loading: boolean;
  /** Refetch without clearing the current data, so live updates do not blank the pane. */
  reload: () => void;
  setData: Dispatch<SetStateAction<T | null>>;
}

/**
 * Runs `load` and tracks its outcome.
 *
 * `load` must be memoised by the caller: its identity *is* the cache key. When
 * it changes the previous request is aborted and the old data is dropped; a
 * `reload()` keeps the old data on screen while the new response is in flight.
 */
export function useAsyncResource<T>(load: Loader<T>): AsyncResource<T> {
  const [settled, setSettled] = useState<Settled<T> | null>(null);
  const [token, setToken] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    let active = true;

    load(controller.signal)
      .then((result) => {
        if (active) setSettled({ loader: load, token, data: result, error: null });
      })
      .catch((cause: unknown) => {
        if (!active || controller.signal.aborted) return;
        setSettled({
          loader: load,
          token,
          data: null,
          error: cause instanceof Error ? cause : new Error(String(cause)),
        });
      });

    return () => {
      active = false;
      controller.abort();
    };
  }, [load, token]);

  const forThisLoader = settled?.loader === load ? settled : null;
  const upToDate = forThisLoader?.token === token ? forThisLoader : null;

  const setData = useCallback<Dispatch<SetStateAction<T | null>>>((update) => {
    setSettled((previous) => {
      if (previous === null) return previous;
      const next = typeof update === 'function' ? (update as (prior: T | null) => T | null)(previous.data) : update;
      return { ...previous, data: next };
    });
  }, []);

  return {
    data: forThisLoader?.data ?? null,
    error: upToDate?.error ?? null,
    loading: upToDate === null,
    reload: useCallback(() => setToken((current) => current + 1), []),
    setData,
  };
}
