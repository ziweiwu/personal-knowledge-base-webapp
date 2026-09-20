import { Link, useLocation } from 'react-router-dom';
import { NotFoundState } from '../components/ui/States';
import { folderRoute } from '../api/paths';

const ROOT_ROUTE_PREFIXES = ['n', 'f', 't', 'trash'];

/** The root id an app address carries, if its first segment is one of ours. */
function rootIdFrom(pathname: string): string | null {
  const [prefix, rootId] = pathname.split('/').filter(Boolean);
  if (!prefix || !rootId || !ROOT_ROUTE_PREFIXES.includes(prefix)) return null;
  return decodeURIComponent(rootId);
}

export function NotFoundPage() {
  const { pathname } = useLocation();
  const rootId = rootIdFrom(pathname);
  return (
    <NotFoundState
      title="Page not found"
      detail={
        <>
          Nothing in this app answers to <code className="state__path">{pathname}</code>.
        </>
      }
    >
      <div className="state__actions">
        <Link className="btn btn--primary" to="/">
          Go home
        </Link>
        {rootId ? (
          <Link className="btn" to={folderRoute(rootId, '')}>
            Open the collection
          </Link>
        ) : null}
      </div>
    </NotFoundState>
  );
}
