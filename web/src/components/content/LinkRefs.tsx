import { Link } from 'react-router-dom';
import { docRoute } from '../../api/paths';
import type { LinkRef } from '../../api/types';

interface LinkRefsProps {
  title: string;
  refs: LinkRef[];
  rootId: string;
  emptyLabel?: string;
}

export function LinkRefs({ title, refs, rootId, emptyLabel }: LinkRefsProps) {
  if (refs.length === 0 && !emptyLabel) return null;

  return (
    <section className="linkrefs" aria-labelledby={`linkrefs-${title.replace(/\s+/g, '-').toLowerCase()}`}>
      <h2 className="linkrefs__title" id={`linkrefs-${title.replace(/\s+/g, '-').toLowerCase()}`}>
        {title} {refs.length > 0 ? `(${refs.length})` : ''}
      </h2>
      {refs.length === 0 ? (
        <p className="state__detail" style={{ textAlign: 'left' }}>
          {emptyLabel}
        </p>
      ) : (
        <ul className="linkrefs__list">
          {refs.map((ref) => (
            <li className="linkrefs__item" key={ref.path}>
              <Link to={docRoute(rootId, ref.path)}>
                {ref.title || ref.path}
                <span className="linkrefs__path">{ref.path}</span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
