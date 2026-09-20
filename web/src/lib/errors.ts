import { ApiRequestError, NetworkError } from '../api/client';

/** The shape of a failure, coarse enough for a screen to pick a tone and a glyph by. */
export type ErrorKind = 'not-found' | 'unauthorized' | 'rate-limited' | 'network' | 'server' | 'unknown';

/** A failure that only needs patience or a retry is a warning; the rest are danger. */
export type ErrorSeverity = 'warning' | 'danger';

export interface ErrorDescription {
  kind: ErrorKind;
  title: string;
  detail: string;
  severity: ErrorSeverity;
}

function isNetworkFailure(error: Error): boolean {
  return error instanceof NetworkError || error.name === 'NetworkError';
}

/** Turns any thrown value into a heading plus a sentence a person can act on. */
export function describeError(error: Error): ErrorDescription {
  if (error instanceof ApiRequestError) {
    if (error.isNotFound) {
      return {
        kind: 'not-found',
        severity: 'danger',
        title: 'Not found',
        detail: "This note or folder isn't here. It may have been moved or renamed.",
      };
    }
    if (error.isUnauthorized) {
      return {
        kind: 'unauthorized',
        severity: 'danger',
        title: 'Signed out',
        detail: 'Your session expired. Sign in again.',
      };
    }
    if (error.isRateLimited)
      return { kind: 'rate-limited', severity: 'warning', title: 'Too many requests', detail: error.message };
    return { kind: 'server', severity: 'danger', title: `Request failed (${error.status})`, detail: error.message };
  }
  if (isNetworkFailure(error)) {
    return {
      kind: 'network',
      severity: 'warning',
      title: "Can't reach the server",
      detail: 'Check the connection, then try again. Nothing you typed has been lost.',
    };
  }
  return {
    kind: 'unknown',
    severity: 'danger',
    title: 'Something went wrong',
    detail: error.message || 'The request could not be completed.',
  };
}
