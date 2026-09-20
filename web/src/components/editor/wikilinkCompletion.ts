import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import type { EditorView } from '@codemirror/view';
import { fetchTree } from '../../api/client';
import type { TreeNode } from '../../api/types';

/** One note the completion can point at: the title Obsidian would link it by, and where it lives. */
export interface NoteTitle {
  title: string;
  path: string;
  folder: string;
  /** The text a link needs: the bare title, or the path when another note shares the title. */
  linkTarget: string;
}

/** `[[` followed by anything that has not yet closed the link, up to the cursor. */
const WIKILINK_OPEN = /\[\[([^\]]*)$/;
const OPEN_LENGTH = '[['.length;
const CLOSE = ']]';
const MAX_OPTIONS = 20;

const MARKDOWN_EXTENSION = /\.md$/i;
const WORD_BOUNDARY = /[\s\-_/.]+/;

/** How well a note answers the query; higher is listed first, zero is left out. */
const SCORE_TITLE_PREFIX = 4;
const SCORE_WORD_PREFIX = 3;
const SCORE_TITLE_CONTAINS = 2;
const SCORE_PATH_CONTAINS = 1;
const SCORE_NONE = 0;

const titlesByRoot = new Map<string, Promise<NoteTitle[]>>();

function collectNotes(nodes: TreeNode[], into: NoteTitle[]): void {
  for (const node of nodes) {
    if (node.isDir) {
      if (node.children) collectNotes(node.children, into);
      continue;
    }
    if (node.kind !== 'markdown' && !MARKDOWN_EXTENSION.test(node.name)) continue;
    const title = node.name.replace(MARKDOWN_EXTENSION, '');
    const folder = node.path.slice(0, node.path.length - node.name.length).replace(/\/$/, '');
    into.push({ title, path: node.path, folder, linkTarget: title });
  }
}

/** Obsidian resolves a shared title by path, so a duplicate is linked the way it would be read. */
function disambiguateTitles(notes: NoteTitle[]): NoteTitle[] {
  const titleCounts = new Map<string, number>();
  for (const note of notes) titleCounts.set(note.title, (titleCounts.get(note.title) ?? 0) + 1);
  return notes.map((note) =>
    (titleCounts.get(note.title) ?? 0) > 1 ? { ...note, linkTarget: note.path.replace(MARKDOWN_EXTENSION, '') } : note,
  );
}

/** The root's notes, fetched once per editor session; a failed fetch is retried next time. */
export function noteTitles(rootId: string): Promise<NoteTitle[]> {
  const known = titlesByRoot.get(rootId);
  if (known) return known;
  const pending = fetchTree(rootId).then((tree) => {
    const notes: NoteTitle[] = [];
    collectNotes(tree, notes);
    return disambiguateTitles(notes);
  });
  pending.catch(() => titlesByRoot.delete(rootId));
  titlesByRoot.set(rootId, pending);
  return pending;
}

function scoreNote(note: NoteTitle, query: string): number {
  if (!query) return SCORE_PATH_CONTAINS;
  const title = note.title.toLowerCase();
  if (title.startsWith(query)) return SCORE_TITLE_PREFIX;
  if (title.split(WORD_BOUNDARY).some((word) => word.startsWith(query))) return SCORE_WORD_PREFIX;
  if (title.includes(query)) return SCORE_TITLE_CONTAINS;
  if (note.path.toLowerCase().includes(query)) return SCORE_PATH_CONTAINS;
  return SCORE_NONE;
}

/** Notes matching the query, best first, ties broken alphabetically so the list is stable. */
export function rankNotes(notes: NoteTitle[], rawQuery: string): NoteTitle[] {
  const query = rawQuery.trim().toLowerCase();
  const scored = notes
    .map((note) => ({ note, score: scoreNote(note, query) }))
    .filter((entry) => entry.score > SCORE_NONE);
  scored.sort((left, right) => right.score - left.score || left.note.title.localeCompare(right.note.title));
  return scored.slice(0, MAX_OPTIONS).map((entry) => entry.note);
}

/** Replace the typed fragment with the title and close the link, unless it is already closed. */
function applyNote(note: NoteTitle) {
  return (view: EditorView, _completion: Completion, from: number, to: number) => {
    const alreadyClosed = view.state.sliceDoc(to, to + CLOSE.length) === CLOSE;
    const insert = alreadyClosed ? note.linkTarget : `${note.linkTarget}${CLOSE}`;
    view.dispatch({
      changes: { from, to, insert },
      selection: { anchor: from + note.linkTarget.length + CLOSE.length },
      userEvent: 'input.complete',
    });
  };
}

function toCompletion(note: NoteTitle): Completion {
  return { label: note.title, detail: note.folder || undefined, type: 'text', apply: applyNote(note) };
}

/**
 * Completes `[[` with the titles of the root's notes, the way Obsidian does. Only the
 * title form is offered here; an alias after `|` or a heading after `#` is typed by hand.
 */
export function wikilinkCompletionSource(rootId: string) {
  return async (context: CompletionContext): Promise<CompletionResult | null> => {
    const match = context.matchBefore(WIKILINK_OPEN);
    if (!match) return null;
    const from = match.from + OPEN_LENGTH;
    const query = context.state.sliceDoc(from, context.pos);
    const notes = await noteTitles(rootId);
    const options = rankNotes(notes, query).map(toCompletion);
    if (options.length === 0) return null;
    // Ranking is done here, so CodeMirror's own fuzzy filter must stay out of the way.
    return { from, options, filter: false };
  };
}
