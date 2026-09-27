import { useEffect, useRef } from 'react';
import { autocompletion, completionKeymap } from '@codemirror/autocomplete';
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { bracketMatching, indentOnInput } from '@codemirror/language';
import { Compartment, EditorState, type Extension } from '@codemirror/state';
import {
  EditorView,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
} from '@codemirror/view';
import { editorTheme, readingHighlighting } from './editorTheme';
import { wikilinkCompletionSource } from './wikilinkCompletion';

interface EditorHandlers {
  onChange: (value: string) => void;
  onSave: () => void;
}
import { insertLink, toggleBold, toggleItalic, toggleTask, type FormatCommand } from './formatting';

/** What the parent gets from onReady: the buffer, plus a way to run formatting commands. */
export interface EditorHandle {
  getValue: () => string;
  setValue: (next: string) => void;
  focus: () => void;
  run: (command: FormatCommand) => void;
}

/**
 * Keep the file's own line ending, so saving a CRLF note does not rewrite every line of it.
 *
 * CodeMirror splits a document on `/\r\n?|\n/` and rejoins it with `state.lineBreak`,
 * which is `\n` unless told otherwise — so a Windows-authored note came back from the
 * editor with every ending changed, including in the lines nobody touched, and the whole
 * file showed as modified in git afterwards (INV-17). Naming the separator makes `\r\n`
 * both what the document is split on and what Enter inserts. A lone `\n` in an otherwise
 * CRLF file then stays inside its line, which is byte preservation rather than a tidy-up.
 */
function lineEndingOf(text: string): Extension[] {
  return text.includes('\r\n') ? [EditorState.lineSeparator.of('\r\n')] : [];
}

/**
 * The buffer as the file should be written, rather than as CodeMirror prints it.
 *
 * `doc.toString()` joins lines with `\n` unconditionally — the separator facet governs
 * what the document is *split* on and what a command inserts, not this — so reading the
 * buffer that way undoes `lineEndingOf` at the last step. `sliceString` takes the
 * separator to use, and `state.lineBreak` is the one this document was built with.
 */
function docText(state: EditorState): string {
  return state.doc.sliceString(0, state.doc.length, state.lineBreak);
}

const FORMATTING_KEYMAP = [
  { key: 'Mod-b', run: toggleBold },
  { key: 'Mod-i', run: toggleItalic },
  { key: 'Mod-k', run: insertLink },
  { key: 'Mod-Shift-c', run: toggleTask },
];

interface CodeMirrorFieldProps {
  initialValue: string;
  /** The root the buffer belongs to; wikilink completion offers that root's notes. */
  rootId: string;
  language: 'markdown' | 'plain';
  theme: 'light' | 'dark';
  onChange: (value: string) => void;
  onSave: () => void;
  /** Called once with the handle so the parent can read, replace and format the buffer. */
  onReady: (handle: EditorHandle) => void;
}

/**
 * Everything but the theme, which lives in a compartment so it can change without a rebuild.
 *
 * A markdown note is edited in the reading view's face, so it carries no line numbers
 * and no active-line band: those belong to code, and they are what makes a page look
 * like a source file. Any other kind is code, and keeps them.
 */
function baseExtensions(language: 'markdown' | 'plain', rootId: string, handlers: { current: EditorHandlers }) {
  const codeChrome: Extension[] =
    language === 'plain' ? [lineNumbers(), highlightActiveLineGutter(), highlightActiveLine()] : [];
  const extensions: Extension[] = [
    ...codeChrome,
    highlightSpecialChars(),
    history(),
    drawSelection(),
    indentOnInput(),
    bracketMatching(),
    readingHighlighting,
    EditorView.lineWrapping,
    keymap.of([
      { key: 'Mod-s', preventDefault: true, run: () => (handlers.current.onSave(), true) },
      ...(language === 'markdown' ? FORMATTING_KEYMAP : []),
      ...completionKeymap,
      indentWithTab,
      ...defaultKeymap,
      ...historyKeymap,
    ]),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) handlers.current.onChange(docText(update.state));
    }),
  ];
  if (language === 'markdown') {
    extensions.push(markdown());
    extensions.push(autocompletion({ override: [wikilinkCompletionSource(rootId)], icons: false }));
  }
  return extensions;
}

/**
 * Thin imperative wrapper around CodeMirror 6. This module is only ever reached
 * through the lazily loaded editor chunk, so none of `@codemirror/*` is in the
 * initial bundle.
 */
export function CodeMirrorField({
  initialValue,
  rootId,
  language,
  theme,
  onChange,
  onSave,
  onReady,
}: CodeMirrorFieldProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const themeCompartment = useRef(new Compartment());

  // Latest callbacks, so recreating the view is never needed just to update one.
  const handlers = useRef({ onChange, onSave, onReady });
  useEffect(() => {
    handlers.current = { onChange, onSave, onReady };
  });

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: initialValue,
        extensions: [
          ...baseExtensions(language, rootId, handlers),
          ...lineEndingOf(initialValue),
          themeCompartment.current.of(editorTheme(theme)),
        ],
      }),
    });
    viewRef.current = view;

    handlers.current.onReady({
      getValue: () => docText(view.state),
      setValue: (next: string) => {
        view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } });
      },
      focus: () => view.focus(),
      run: (command) => command(view),
    });

    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // The document is seeded once; later content changes go through the handle.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [language, rootId]);

  useEffect(() => {
    viewRef.current?.dispatch({
      effects: themeCompartment.current.reconfigure(editorTheme(theme)),
    });
  }, [theme]);

  return <div className={`editor__host editor__host--${language === 'markdown' ? 'prose' : 'code'}`} ref={hostRef} />;
}
