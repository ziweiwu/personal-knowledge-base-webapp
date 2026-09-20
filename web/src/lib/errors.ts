import { ApiRequestError } from '../api/client';

/** The shape of a failure, coarse enough for a screen to pick a tone and a glyph by. */
export type ErrorKind = 'not-found' | 'unauthorized' | 'rate-limited' | 'network' | 'server' | 'unknown';

export interface ErrorDescription {
  kind: ErrorKind;
  title: string;
  detail: string;
}

/** A fetch that never reached the server rejects with a TypeError rather than a status. */
function isNetworkFailure(error: Error): boolean {
  return error instanceof TypeError || error.name === 'NetworkError';
}

/** Turns any thrown value into a heading plus a sentence a person can act on. */
export function describeError(error: Error): ErrorDescription {
  if (error instanceof ApiRequestError) {
    if (error.isNotFound) {
      return {
        kind: 'not-found',
        title: 'Not found',
        detail: "This note or folder isn't here. It may have been moved or renamed.",
      };
    }
    if (error.isUnauthorized) {
      return { kind: 'unauthorized', title: 'Signed out', detail: 'Your session expired. Sign in again.' };
    }
    if (error.isRateLimited) return { kind: 'rate-limited', title: 'Too many requests', detail: error.message };
    return { kind: 'server', title: `Request failed (${error.status})`, detail: error.message };
  }
  if (isNetworkFailure(error)) {
    return {
      kind: 'network',
      title: "Can't reach the server",
      detail: 'Check the connection, then try again. Nothing you typed has been lost.',
    };
  }
  return {
    kind: 'unknown',
    title: 'Something went wrong',
    detail: error.message || 'The request could not be completed.',
  };
}
