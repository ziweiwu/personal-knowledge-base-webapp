/**
 * Find-in-page over rendered prose: wraps every match in a `<mark>` and takes
 * the marks out again, leaving the DOM as it was.
 *
 * Matches are found per text node, so a phrase split across an inline element
 * (`**bold** word`) is not found. That is the same limit as the browser's own
 * find on such markup, and it keeps the wrapping a pure split-and-wrap that
 * `clearHighlights` can undo exactly.
 */

export const HIT_CLASS = 'find-hit';
export const CURRENT_CLASS = 'find-hit--current';

/** A `<mark>` is an HTML element; inside these it would be foreign and invalid. */
const SKIPPED_ANCESTORS = 'script, style, textarea, svg, math';

function textNodes(root: HTMLElement): Text[] {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) =>
      node.parentElement?.closest(SKIPPED_ANCESTORS) || !node.nodeValue?.trim()
        ? NodeFilter.FILTER_REJECT
        : NodeFilter.FILTER_ACCEPT,
  });
  const nodes: Text[] = [];
  for (let node = walker.nextNode(); node; node = walker.nextNode()) nodes.push(node as Text);
  return nodes;
}

/** Start offsets of every case-insensitive, non-overlapping occurrence. */
function occurrences(haystack: string, needle: string): number[] {
  const lowerHaystack = haystack.toLowerCase();
  const lowerNeedle = needle.toLowerCase();
  const starts: number[] = [];
  let from = lowerHaystack.indexOf(lowerNeedle);
  while (from !== -1) {
    starts.push(from);
    from = lowerHaystack.indexOf(lowerNeedle, from + lowerNeedle.length);
  }
  return starts;
}

function wrapMatches(node: Text, needle: string): HTMLElement[] {
  const marks: HTMLElement[] = [];
  let rest = node;
  let consumed = 0;
  for (const start of occurrences(node.nodeValue ?? '', needle)) {
    const hit = rest.splitText(start - consumed);
    rest = hit.splitText(needle.length);
    consumed = start + needle.length;
    const mark = document.createElement('mark');
    mark.className = HIT_CLASS;
    hit.replaceWith(mark);
    mark.append(hit);
    marks.push(mark);
  }
  return marks;
}

/** Wrap every match of `query` under `root`; returns the marks in document order. */
export function highlightMatches(root: HTMLElement, query: string): HTMLElement[] {
  if (!query) return [];
  return textNodes(root).flatMap((node) => wrapMatches(node, query));
}

/** Undo `highlightMatches`, merging the split text nodes back together. */
export function clearHighlights(root: HTMLElement): void {
  const parents = new Set<Node>();
  for (const mark of root.querySelectorAll(`mark.${HIT_CLASS}`)) {
    if (mark.parentNode) parents.add(mark.parentNode);
    mark.replaceWith(...mark.childNodes);
  }
  for (const parent of parents) parent.normalize();
}

/** Make one mark the current hit and bring it into view. */
export function focusMatch(marks: HTMLElement[], index: number): void {
  marks.forEach((mark, at) => mark.classList.toggle(CURRENT_CLASS, at === index));
  const current = marks[index];
  if (!current) return;
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  current.scrollIntoView({ block: 'center', behavior: reduceMotion ? 'auto' : 'smooth' });
}
