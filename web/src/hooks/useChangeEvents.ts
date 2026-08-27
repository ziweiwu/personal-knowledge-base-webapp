import { useEffect, useSyncExternalStore } from 'react';
import { changeStream, type ConnectionState } from '../api/events';
import type { ChangeEvent } from '../api/types';

/** Calls `listener` for every change event the server pushes. */
export function useChangeEvents(listener: (event: ChangeEvent) => void): void {
  useEffect(() => changeStream.subscribe(listener), [listener]);
}

export function useConnectionState(): ConnectionState {
  return useSyncExternalStore(
    (onChange) => changeStream.observeState(() => onChange()),
    () => changeStream.getState(),
    () => 'connecting' as const,
  );
}
