/** Circular list navigation: stepping past either end lands on the other, and an empty list stays at 0. */
export function wrapIndex(index: number, count: number): number {
  return count === 0 ? 0 : ((index % count) + count) % count;
}
