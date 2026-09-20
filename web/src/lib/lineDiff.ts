/**
 * A line-level diff small enough to live in the client: the longest common
 * subsequence of lines, computed with the classic dynamic-programming table.
 * Lines on either side that are not part of that subsequence are "changed",
 * which covers added, removed and modified lines alike.
 */

export type LineKind = 'same' | 'changed';

export interface DiffLine {
  text: string;
  kind: LineKind;
}

export interface LineDiff {
  /** Lines of the first text, each marked. */
  left: DiffLine[];
  /** Lines of the second text, each marked. */
  right: DiffLine[];
  /** Changed lines across both sides. */
  changed: number;
  /** True when either side was too long to compare and nothing was marked. */
  tooLarge: boolean;
}

/** The LCS table is quadratic (8 MB at this size); past it, plain panes beat a stalled dialog. */
export const MAX_DIFF_LINES = 2000;

export function splitLines(text: string): string[] {
  const lines = text.split('\n');
  // A trailing newline is a line ending, not an extra empty line.
  if (lines.length > 1 && lines[lines.length - 1] === '') lines.pop();
  return lines;
}

function unmarked(lines: string[]): DiffLine[] {
  return lines.map((text) => ({ text, kind: 'same' }));
}

/** Length of the longest common subsequence of every prefix pair, as a flat row-major table. */
function lcsTable(left: string[], right: string[]): Uint16Array {
  const width = right.length + 1;
  const table = new Uint16Array((left.length + 1) * width);
  for (let row = left.length - 1; row >= 0; row -= 1) {
    for (let column = right.length - 1; column >= 0; column -= 1) {
      const here = row * width + column;
      table[here] =
        left[row] === right[column] ? table[here + width + 1] + 1 : Math.max(table[here + width], table[here + 1]);
    }
  }
  return table;
}

/** Walk the table from the top-left corner, keeping matched lines and marking the rest. */
function markLines(left: string[], right: string[], table: Uint16Array): LineDiff {
  const width = right.length + 1;
  const leftMarked: DiffLine[] = [];
  const rightMarked: DiffLine[] = [];
  let row = 0;
  let column = 0;
  while (row < left.length && column < right.length) {
    if (left[row] === right[column]) {
      leftMarked.push({ text: left[row], kind: 'same' });
      rightMarked.push({ text: right[column], kind: 'same' });
      row += 1;
      column += 1;
    } else if (table[(row + 1) * width + column] >= table[row * width + column + 1]) {
      leftMarked.push({ text: left[row], kind: 'changed' });
      row += 1;
    } else {
      rightMarked.push({ text: right[column], kind: 'changed' });
      column += 1;
    }
  }
  for (; row < left.length; row += 1) leftMarked.push({ text: left[row], kind: 'changed' });
  for (; column < right.length; column += 1) rightMarked.push({ text: right[column], kind: 'changed' });
  const changed = [...leftMarked, ...rightMarked].filter((line) => line.kind === 'changed').length;
  return { left: leftMarked, right: rightMarked, changed, tooLarge: false };
}

export function diffLines(leftText: string, rightText: string): LineDiff {
  const left = splitLines(leftText);
  const right = splitLines(rightText);
  if (left.length > MAX_DIFF_LINES || right.length > MAX_DIFF_LINES) {
    return { left: unmarked(left), right: unmarked(right), changed: 0, tooLarge: true };
  }
  return markLines(left, right, lcsTable(left, right));
}
