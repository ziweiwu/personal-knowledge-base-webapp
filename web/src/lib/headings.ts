import type { Heading } from '../api/types';

/** One heading is the title; a list needs at least two to be worth a jump. */
export function hasTableOfContents(headings: Heading[]): boolean {
  return headings.length >= 2;
}
