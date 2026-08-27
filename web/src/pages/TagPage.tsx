import { useCallback, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { fetchTagged } from '../api/client';
import { docRoute } from '../api/paths';
import { useAsyncResource } from '../hooks/useAsyncResource';
import { formatRelative, formatSize, kindIcon, kindLabel } from '../lib/format';
import { EmptyState, ErrorState, LoadingState } from '../components/ui/States';

interface Props {
  rootId: string;
  tag: string;
  onTitleChange: (title: string) => void;
}

/**
 * Every document carrying a tag.
 *
 * Rendered tags link here. Without this page they pointed at a route that did not exist,
 * so an index listing dozens of tags was dozens of dead ends.
 */
export function TagPage({ rootId, tag, onTitleChange }: Props) {
  const load = useCallback((signal: AbortSignal) => fetchTagged(rootId, tag, signal), [rootId, tag]);
  const resource = useAsyncResource(load);

  useEffect(() => {
    onTitleChange(`#${tag}`);
  }, [tag, onTitleChange]);

  if (resource.error) return <ErrorState error={resource.error} onRetry={resource.reload} />;
  if (!resource.data) return resource.loading ? <LoadingState label="Loading tagged documents…" /> : null;

  const documents = resource.data;

  return (
    <div className="doc">
      <div className="doc__inner">
        <h1 className="doc__title">#{tag}</h1>
        {documents.length === 0 ? (
          <EmptyState
            title={`Nothing is tagged #${tag}`}
            detail="No document in this collection carries that tag."
          />
        ) : (
          <>
            <h2 className="folder__section-title" id="tagged-documents">
              {documents.length === 1 ? '1 document' : `${documents.length} documents`}
            </h2>
            <ul className="entries" aria-labelledby="tagged-documents">
              {documents.map((document) => (
                <li className="entries__item" key={document.path}>
                  <Link className="entries__link" to={docRoute(rootId, document.path)}>
                    <span className="kind-icon" aria-hidden="true">
                      {kindIcon(document.kind)}
                    </span>
                    <span className="entries__name">
                      {document.title}
                      <span className="entries__sub">
                        {kindLabel(document.kind)} · {document.path}
                      </span>
                    </span>
                    <span className="entries__meta">
                      {formatSize(document.size)} · {formatRelative(document.mtimeMs)}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </div>
  );
}
