import { Link } from 'react-router-dom';
import { folderRoute } from '../../api/paths';
import type { VaultMode } from './VaultLayout';

interface BreadcrumbsProps {
  rootId: string;
  rootName: string;
  path: string;
  mode: VaultMode;
}

/**
 * Only folder segments become links.
 *
 * A document's own name is already the page title above the trail, so it is
 * dropped. A tag is not a path at all: its segments name no folder, and
 * linking them produced a plausible-looking `/f/…` address that always 404s,
 * so the tag is rendered as the current-page label instead.
 */
export function Breadcrumbs({ rootId, rootName, path, mode }: BreadcrumbsProps) {
  const segments = path.split('/').filter(Boolean);
  const folders = mode === 'tag' ? [] : mode === 'doc' ? segments.slice(0, -1) : segments;
  const current = mode === 'tag' ? path : null;

  return (
    <nav aria-label="Breadcrumb" className="topbar__crumbs">
      <Link to={folderRoute(rootId, '')}>{rootName}</Link>
      {folders.map((segment, index) => (
        <span key={`${segment}-${index}`}>
          {' / '}
          <Link to={folderRoute(rootId, folders.slice(0, index + 1).join('/'))}>{segment}</Link>
        </span>
      ))}
      {current ? (
        <span>
          {' / '}
          <span aria-current="page">{current}</span>
        </span>
      ) : null}
    </nav>
  );
}
