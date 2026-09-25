/** The hues a tag chip may take; each pairs a `--<tone>` ink with a `--<tone>-subtle` ground in tokens.css. */
export const TAG_TONES = ['accent', 'teal', 'violet', 'success'] as const;

export type TagTone = (typeof TAG_TONES)[number];

/**
 * A tag always gets the same hue, so a reader learns to spot it across notes; the hue is
 * spread over the four tones by the tag's letters rather than its order in one note.
 */
export function tagTone(tag: string): TagTone {
  let sum = 0;
  for (const char of tag) sum += char.codePointAt(0) ?? 0;
  return TAG_TONES[sum % TAG_TONES.length];
}
