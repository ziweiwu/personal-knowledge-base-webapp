export interface SnippetPart {
  text: string;
  match: boolean;
}

/**
 * Splits a `SearchHit.snippet` on its `**match**` markers.
 *
 * The result is rendered as elements, never as HTML — search text is derived
 * from file contents and must not be able to inject markup.
 */
export function parseSnippet(snippet: string): SnippetPart[] {
  const parts: SnippetPart[] = [];
  let cursor = 0;

  while (cursor < snippet.length) {
    const open = snippet.indexOf('**', cursor);
    if (open === -1) break;
    const close = snippet.indexOf('**', open + 2);
    if (close === -1) break;

    if (open > cursor) parts.push({ text: snippet.slice(cursor, open), match: false });
    const matched = snippet.slice(open + 2, close);
    if (matched) parts.push({ text: matched, match: true });
    cursor = close + 2;
  }

  if (cursor < snippet.length) parts.push({ text: snippet.slice(cursor), match: false });
  return parts.length > 0 ? parts : [{ text: snippet, match: false }];
}
