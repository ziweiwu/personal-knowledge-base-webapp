import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { tags } from '@lezer/highlight';

const SELECTION =
  '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, ' +
  '.cm-selectionBackground, .cm-content ::selection';

/**
 * The editor wears the reading view's clothes: every colour is a palette token, so
 * the buffer looks like the page it will become in both themes, and switching theme
 * only has to tell CodeMirror which side it is on (that decides its built-in
 * defaults such as the drop cursor) rather than swap a whole theme.
 */
export function editorTheme(theme: 'light' | 'dark'): Extension {
  return EditorView.theme(
    {
      '&': { color: 'var(--fg)', backgroundColor: 'var(--bg)' },
      '.cm-content': { caretColor: 'var(--accent)' },
      '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--accent)' },
      [SELECTION]: { backgroundColor: 'var(--accent-subtle)' },
      '.cm-activeLine': { backgroundColor: 'var(--bg-hover)' },
      '.cm-gutters': {
        backgroundColor: 'var(--bg)',
        color: 'var(--fg-faint)',
        borderRight: '1px solid var(--border)',
      },
      '.cm-activeLineGutter': { backgroundColor: 'var(--bg-hover)', color: 'var(--fg-muted)' },
      '&.cm-focused .cm-matchingBracket, &.cm-focused .cm-nonmatchingBracket': {
        backgroundColor: 'var(--accent-subtle)',
        outline: 'none',
      },
      '.cm-specialChar': { color: 'var(--danger)' },
      '.cm-tooltip': { backgroundColor: 'var(--bg-elevated)', color: 'var(--fg)', borderColor: 'var(--border)' },
    },
    { dark: theme === 'dark' },
  );
}

/**
 * Markdown highlighting for a note that is being read as much as written: structure
 * is shown through weight, slant and size, the way the rendered page shows it, and
 * the markup characters themselves recede. Code keeps its own face so a fenced
 * block reads as code inside serif prose, exactly as it does in the reading view.
 */
const readingHighlight = HighlightStyle.define([
  { tag: tags.heading1, fontWeight: '700', fontSize: '1.5em' },
  { tag: tags.heading2, fontWeight: '700', fontSize: '1.3em' },
  { tag: tags.heading3, fontWeight: '700', fontSize: '1.15em' },
  { tag: [tags.heading4, tags.heading5, tags.heading6], fontWeight: '700' },
  { tag: tags.strong, fontWeight: '700' },
  { tag: tags.emphasis, fontStyle: 'italic' },
  { tag: tags.strikethrough, textDecoration: 'line-through' },
  { tag: [tags.link, tags.url], color: 'var(--accent-fg)' },
  { tag: tags.quote, color: 'var(--fg-muted)' },
  { tag: [tags.processingInstruction, tags.contentSeparator, tags.comment], color: 'var(--fg-faint)' },
  { tag: [tags.labelName, tags.string], color: 'var(--fg-muted)' },
  { tag: tags.monospace, fontFamily: 'var(--font-mono)', fontSize: '0.88em' },
]);

export const readingHighlighting: Extension = syntaxHighlighting(readingHighlight, { fallback: true });
