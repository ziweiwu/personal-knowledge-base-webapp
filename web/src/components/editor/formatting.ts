import { EditorSelection, type ChangeSpec, type EditorState, type SelectionRange } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';

/** A CodeMirror command: applies to the view and reports whether it did anything. */
export type FormatCommand = (view: EditorView) => boolean;

interface WrapMarks {
  open: string;
  close: string;
}

/** Headings cycle # → ## → ### → none, since deeper levels are rare enough to type. */
const MAX_CYCLED_HEADING_LEVEL = 3;
const HEADING_PATTERN = /^(#{1,6})\s+/;
const BULLET_PATTERN = /^(\s*)- (?!\[[ xX]\] )/;
const TASK_PATTERN = /^(\s*)- \[([ xX])\] /;
const INDENT_PATTERN = /^\s*/;

function isWrappedInside(text: string, marks: WrapMarks): boolean {
  const { open, close } = marks;
  return text.length >= open.length + close.length && text.startsWith(open) && text.endsWith(close);
}

function isWrappedOutside(state: EditorState, range: SelectionRange, marks: WrapMarks): boolean {
  const { open, close } = marks;
  const before = state.sliceDoc(Math.max(0, range.from - open.length), range.from);
  const after = state.sliceDoc(range.to, Math.min(state.doc.length, range.to + close.length));
  return range.from >= open.length && before === open && after === close;
}

function wrapRange(state: EditorState, range: SelectionRange, marks: WrapMarks) {
  const { open, close } = marks;
  const text = state.sliceDoc(range.from, range.to);
  if (isWrappedInside(text, marks)) {
    const inner = text.slice(open.length, text.length - close.length);
    return {
      changes: { from: range.from, to: range.to, insert: inner },
      range: EditorSelection.range(range.from, range.from + inner.length),
    };
  }
  if (isWrappedOutside(state, range, marks)) {
    return {
      changes: [
        { from: range.from - open.length, to: range.from, insert: '' },
        { from: range.to, to: range.to + close.length, insert: '' },
      ],
      range: EditorSelection.range(range.from - open.length, range.to - open.length),
    };
  }
  return {
    changes: { from: range.from, to: range.to, insert: `${open}${text}${close}` },
    range: EditorSelection.range(range.from + open.length, range.to + open.length),
  };
}

/** Wrap each selection in the marks, or unwrap it when the marks are already there. */
function toggleWrap(marks: WrapMarks): FormatCommand {
  return (view) => {
    view.dispatch(
      view.state.changeByRange((range) => wrapRange(view.state, range, marks)),
      { scrollIntoView: true, userEvent: 'input.format' },
    );
    view.focus();
    return true;
  };
}

export const toggleBold = toggleWrap({ open: '**', close: '**' });
export const toggleItalic = toggleWrap({ open: '*', close: '*' });
export const toggleInlineCode = toggleWrap({ open: '`', close: '`' });
export const toggleWikilink = toggleWrap({ open: '[[', close: ']]' });

/**
 * `[selection](url)` with the cursor parked where the url goes. With nothing selected the
 * cursor lands in the link text instead, since that is what gets typed first.
 */
export const insertLink: FormatCommand = (view) => {
  const urlOffset = '[]('.length;
  view.dispatch(
    view.state.changeByRange((range) => {
      const text = view.state.sliceDoc(range.from, range.to);
      const cursor = text ? range.from + text.length + urlOffset : range.from + '['.length;
      return {
        changes: { from: range.from, to: range.to, insert: `[${text}]()` },
        range: EditorSelection.cursor(cursor),
      };
    }),
    { scrollIntoView: true, userEvent: 'input.format' },
  );
  view.focus();
  return true;
};

/** Every line touched by any selection, once, in document order. */
function selectedLineNumbers(state: EditorState): number[] {
  const numbers = new Set<number>();
  for (const range of state.selection.ranges) {
    const first = state.doc.lineAt(range.from).number;
    const last = state.doc.lineAt(range.to).number;
    for (let line = first; line <= last; line += 1) numbers.add(line);
  }
  return [...numbers].sort((left, right) => left - right);
}

/** Rewrite each selected line; the selection follows the text it was on. */
function rewriteLines(rewrite: (line: string) => string): FormatCommand {
  return (view) => {
    const { state } = view;
    const changes: ChangeSpec[] = [];
    for (const number of selectedLineNumbers(state)) {
      const line = state.doc.line(number);
      const next = rewrite(line.text);
      if (next !== line.text) changes.push({ from: line.from, to: line.to, insert: next });
    }
    if (changes.length === 0) return false;
    const changeSet = state.changes(changes);
    view.dispatch({
      changes: changeSet,
      selection: state.selection.map(changeSet),
      scrollIntoView: true,
      userEvent: 'input.format',
    });
    view.focus();
    return true;
  };
}

function cycleHeadingLine(text: string): string {
  const match = HEADING_PATTERN.exec(text);
  const level = match ? match[1].length : 0;
  const body = match ? text.slice(match[0].length) : text;
  if (level >= MAX_CYCLED_HEADING_LEVEL) return body;
  return `${'#'.repeat(level + 1)} ${body}`;
}

function toggleTaskLine(text: string): string {
  const task = TASK_PATTERN.exec(text);
  if (task) {
    const checked = task[2] !== ' ';
    return `${task[1]}- [${checked ? ' ' : 'x'}] ${text.slice(task[0].length)}`;
  }
  const bullet = BULLET_PATTERN.exec(text);
  if (bullet) return `${bullet[1]}- [ ] ${text.slice(bullet[0].length)}`;
  const indent = INDENT_PATTERN.exec(text)?.[0] ?? '';
  return `${indent}- [ ] ${text.slice(indent.length)}`;
}

function toggleBulletLine(text: string): string {
  const bullet = BULLET_PATTERN.exec(text);
  if (bullet) return `${bullet[1]}${text.slice(bullet[0].length)}`;
  const indent = INDENT_PATTERN.exec(text)?.[0] ?? '';
  return `${indent}- ${text.slice(indent.length)}`;
}

export const cycleHeading = rewriteLines(cycleHeadingLine);
export const toggleTask = rewriteLines(toggleTaskLine);
export const toggleBullet = rewriteLines(toggleBulletLine);
