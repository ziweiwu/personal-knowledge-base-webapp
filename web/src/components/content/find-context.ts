import { createContext } from 'react';

export interface FindState {
  open: boolean;
  close: () => void;
}

/**
 * Whether the document page wants a find bar over its prose. Provided by the
 * page, read by HtmlContent, which owns the prose container the bar searches.
 * Absent (null) wherever prose is rendered outside a document page, such as a
 * folder's index note.
 */
export const FindContext = createContext<FindState | null>(null);
