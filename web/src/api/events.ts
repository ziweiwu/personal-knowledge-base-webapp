import type { ChangeEvent } from './types';
import { CLIENT_ORIGIN } from './client';
import { API_BASE } from './paths';

export type ConnectionState = 'connecting' | 'live' | 'down';

type ChangeListener = (event: ChangeEvent) => void;
type StateListener = (state: ConnectionState) => void;

function isChangeEvent(value: unknown): value is ChangeEvent {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<ChangeEvent>;
  return typeof candidate.rootId === 'string' && Array.isArray(candidate.paths);
}

/**
 * Single shared SSE subscription to `/api/events`.
 *
 * `EventSource` reconnects on its own, but it reports no HTTP status, so a
 * stream that keeps failing (an expired cookie, say) would retry forever. After
 * a handful of consecutive failures we stop and report `down`; the UI offers a
 * manual reconnect and a full reload recovers the 401 redirect.
 */
class ChangeStream {
  private source: EventSource | null = null;
  private readonly changeListeners = new Set<ChangeListener>();
  private readonly stateListeners = new Set<StateListener>();
  private state: ConnectionState = 'connecting';
  private consecutiveFailures = 0;
  private stopped = false;

  private static readonly MAX_CONSECUTIVE_FAILURES = 6;

  subscribe(listener: ChangeListener): () => void {
    this.changeListeners.add(listener);
    this.ensureOpen();
    return () => {
      this.changeListeners.delete(listener);
      if (this.changeListeners.size === 0 && this.stateListeners.size === 0) this.close();
    };
  }

  observeState(listener: StateListener): () => void {
    this.stateListeners.add(listener);
    listener(this.state);
    return () => {
      this.stateListeners.delete(listener);
    };
  }

  getState(): ConnectionState {
    return this.state;
  }

  reconnect(): void {
    this.close();
    this.stopped = false;
    this.consecutiveFailures = 0;
    this.ensureOpen();
  }

  private ensureOpen(): void {
    if (this.source || this.stopped || typeof EventSource === 'undefined') return;
    this.setState('connecting');

    const source = new EventSource(`${API_BASE}/events`, { withCredentials: true });
    this.source = source;

    const onData = (event: MessageEvent<string>) => this.dispatch(event.data);
    source.onopen = () => this.markLive();
    source.onmessage = onData;
    // Servers commonly name the event; listen for both shapes.
    source.addEventListener('change', onData as EventListener);
    source.onerror = () => this.recordFailure(source);
  }

  private markLive(): void {
    this.consecutiveFailures = 0;
    this.setState('live');
  }

  /** Fans one raw SSE payload out to the subscribers, dropping our own echo. */
  private dispatch(payload: string): void {
    this.markLive();
    let parsed: unknown;
    try {
      parsed = JSON.parse(payload);
    } catch {
      return;
    }
    if (!isChangeEvent(parsed)) return;
    if (parsed.origin !== null && parsed.origin === CLIENT_ORIGIN) return;
    for (const listener of this.changeListeners) listener(parsed);
  }

  private recordFailure(source: EventSource): void {
    if (source.readyState === EventSource.CLOSED) {
      this.source = null;
    }
    this.consecutiveFailures += 1;
    this.setState('down');
    if (this.consecutiveFailures < ChangeStream.MAX_CONSECUTIVE_FAILURES) return;
    this.stopped = true;
    this.close();
    this.setState('down');
  }

  private close(): void {
    this.source?.close();
    this.source = null;
  }

  private setState(next: ConnectionState): void {
    if (this.state === next) return;
    this.state = next;
    for (const listener of this.stateListeners) listener(next);
  }
}

export const changeStream = new ChangeStream();

/** True when a change event touches `path` or anything beneath it. */
export function eventTouches(event: ChangeEvent, rootId: string, path: string): boolean {
  if (event.rootId !== rootId) return false;
  const target = path.replace(/^\/+|\/+$/g, '');
  return event.paths.some((changed) => {
    const clean = changed.replace(/^\/+|\/+$/g, '');
    if (clean === target) return true;
    if (target === '') return true;
    return clean.startsWith(`${target}/`) || target.startsWith(`${clean}/`);
  });
}
