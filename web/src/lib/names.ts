/**
 * Whether a name can be written back into a wikilink.
 *
 * A rename rewrites every inbound `[[link]]` by substituting the new name between the
 * brackets, so a name carrying wikilink syntax produces a link that still parses and
 * points somewhere else. The server refuses these outright — it is the boundary, and this
 * check does not stand in for it. This exists so the dialog can say so before the user
 * commits, rather than after a round trip that reported a rename as successful.
 *
 * The set mirrors `LINK_BREAKING_CHARS` in `crates/kbviewer-core/src/links.rs`, which
 * documents why each character is on it.
 */
const LINK_BREAKING = /["#:[\]^|]/;

/**
 * What is wrong with this name, or `null` when nothing is.
 *
 * Only meaningful in a wikilinks collection; plain markdown rewrites no links, so the
 * caller leaves this validator off entirely there rather than asking it to say nothing.
 */
export function nameProblem(name: string): string | null {
  const found = LINK_BREAKING.exec(name);
  if (!found) return null;
  return (
    `A name cannot contain “${found[0]}” in this collection — a link pointing at it ` +
    'would read that character as syntax and find a different note.'
  );
}
