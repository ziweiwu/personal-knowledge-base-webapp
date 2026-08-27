import { Link } from 'react-router-dom';
import { folderRoute } from '../../api/paths';

interface BreadcrumbsProps {
  rootId: string;
  rootName: string;
  path: string;
  /** The last segment is the open document and is not itself a folder link. */
  lastIsDocument: boolean;
}

export function Breadcrumbs({ rootId, rootName, path, lastIsDocument }: BreadcrumbsProps) {
  const segments = path.split('/').filter(Boolean);
  const folders = lastIsDocument ? segments.slice(0, -1) : segments;

  return (
    <nav aria-label="Breadcrumb" className="topbar__crumbs">
      <Link to={folderRoute(rootId, '')}>{rootName}</Link>
      {folders.map((segment, index) => (
        <span key={`${segment}-${index}`}>
          {' / '}
          <Link to={folderRoute(rootId, folders.slice(0, index + 1).join('/'))}>{segment}</Link>
        </span>
      ))}
    </nav>
  );
}
