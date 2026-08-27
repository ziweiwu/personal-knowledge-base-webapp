import { ApiRequestError } from '../api/client';

/** Turns any thrown value into a heading plus a sentence a person can act on. */
export function describeError(error: Error): { title: string; detail: string } {
  if (error instanceof ApiRequestError) {
    if (error.isNotFound) return { title: 'Not found', detail: 'That file or folder is no longer here.' };
    if (error.isUnauthorized) return { title: 'Signed out', detail: 'Your session expired. Sign in again.' };
    if (error.isRateLimited) return { title: 'Too many requests', detail: error.message };
    return { title: `Request failed (${error.status})`, detail: error.message };
  }
  return { title: 'Something went wrong', detail: error.message || 'The request could not be completed.' };
}
