import { useEffect, useRef } from 'react';
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { bracketMatching, defaultHighlightStyle, indentOnInput, syntaxHighlighting } from '@codemirror/language';
import { Compartment, EditorState, type Extension } from '@codemirror/state';
import { oneDark } from '@codemirror/theme-one-dark';
import {
  EditorView,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
} from '@codemirror/view';

interface CodeMirrorFieldProps {
  initialValue: string;
  language: 'markdown' | 'plain';
  theme: 'light' | 'dark';
  onChange: (value: string) => void;
  onSave: () => void;
  /** Called once with a getter/setter so the parent can read and replace the buffer. */
  onReady: (handle: { getValue: () => string; setValue: (next: string) => void }) => void;
}

/**
 * Thin imperative wrapper around CodeMirror 6. This module is only ever reached
 * through the lazily loaded editor chunk, so none of `@codemirror/*` is in the
 * initial bundle.
 */
export function CodeMirrorField({ initialValue, language, theme, onChange, onSave, onReady }: CodeMirrorFieldProps) {
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

    const baseExtensions: Extension[] = [
      lineNumbers(),
      highlightActiveLineGutter(),
      highlightSpecialChars(),
      history(),
      drawSelection(),
      indentOnInput(),
      bracketMatching(),
      highlightActiveLine(),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      EditorView.lineWrapping,
      keymap.of([
        { key: 'Mod-s', preventDefault: true, run: () => (handlers.current.onSave(), true) },
        indentWithTab,
        ...defaultKeymap,
        ...historyKeymap,
      ]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) handlers.current.onChange(update.state.doc.toString());
      }),
    ];
    if (language === 'markdown') baseExtensions.push(markdown());

    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: initialValue,
        extensions: [...baseExtensions, themeCompartment.current.of(theme === 'dark' ? oneDark : [])],
      }),
    });
    viewRef.current = view;

    handlers.current.onReady({
      getValue: () => view.state.doc.toString(),
      setValue: (next: string) => {
        view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } });
      },
    });

    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // The document is seeded once; later content changes go through the handle.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [language]);

  useEffect(() => {
    viewRef.current?.dispatch({
      effects: themeCompartment.current.reconfigure(theme === 'dark' ? oneDark : []),
    });
  }, [theme]);

  return <div className="editor__host" ref={hostRef} />;
}
