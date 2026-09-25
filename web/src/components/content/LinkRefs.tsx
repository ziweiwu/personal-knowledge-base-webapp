import type { ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { docRoute } from '../../api/paths';
import type { LinkRef } from '../../api/types';

/** The note the list belongs to, so a backlink's snippet can show which link points here. */
export interface LinkSubject {
  path: string;
  title: string;
}

interface LinkRefsProps {
  title: string;
  refs: LinkRef[];
  rootId: string;
  subject?: LinkSubject;
  emptyLabel?: string;
}

const WIKILINK = /!?\[\[([^\]]*)\]\]/g;

/** Every spelling a wikilink may use for the subject, lower-cased: title, path, path and basename without extension. */
function subjectNames(subject: LinkSubject): Set<string> {
  const bare = subject.path.replace(/\.[^./]+$/, '');
  return new Set([subject.title, subject.path, bare, bare.split('/').pop() ?? bare].map((name) => name.toLowerCase()));
}

function pointsAt(inner: string, names: Set<string>): boolean {
  const target = inner.split('|')[0].split('#')[0].trim().replace(/^\.\//, '').toLowerCase();
  return names.has(target);
}

/**
 * The snippet with the wikilink that points at the subject wrapped in `<mark>`. The
 * match is case-insensitive, as Obsidian's own resolution is; a snippet whose link is
 * spelled some other way, or a markdown link, is shown unmarked rather than guessed at.
 */
function markLink(context: string, subject: LinkSubject | undefined): ReactNode {
  if (!subject) return context;
  const names = subjectNames(subject);
  for (const match of context.matchAll(WIKILINK)) {
    if (!pointsAt(match[1], names)) continue;
    const start = match.index;
    const end = start + match[0].length;
    return (
      <>
        {context.slice(0, start)}
        <mark>{match[0]}</mark>
        {context.slice(end)}
      </>
    );
  }
  return context;
}

export function LinkRefs({ title, refs, rootId, subject, emptyLabel }: LinkRefsProps) {
  if (refs.length === 0 && !emptyLabel) return null;
  const headingId = `linkrefs-${title.replace(/\s+/g, '-').toLowerCase()}`;
  return (
    <section className="linkrefs" aria-labelledby={headingId}>
      <h2 className="linkrefs__title" id={headingId}>
        {title} {refs.length > 0 ? `(${refs.length})` : ''}
      </h2>
      {refs.length === 0 ? (
        <p className="state__detail state__detail--start">{emptyLabel}</p>
      ) : (
        <ul className="linkrefs__list">
          {refs.map((ref) => (
            <li className="linkrefs__item" key={ref.path}>
              <Link to={docRoute(rootId, ref.path)}>
                {ref.title || ref.path}
                <span className="linkrefs__path">{ref.path}</span>
              </Link>
              {ref.context ? (
                <blockquote className="linkrefs__context">{markLink(ref.context, subject)}</blockquote>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
