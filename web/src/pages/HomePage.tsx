import { Navigate } from 'react-router-dom';
import { folderRoute } from '../api/paths';
import { ErrorState, LoadingState } from '../components/ui/States';
import { useRoots } from '../state/roots-context';

/** `/` has no content of its own: it forwards to the first configured root. */
export function HomePage() {
  const { roots, loading, error, reload } = useRoots();

  if (error) return <ErrorState error={error} onRetry={reload} />;
  if (loading) return <LoadingState label="Loading your collections…" />;
  if (roots.length === 0) {
    return (
      <div className="state">
        <p className="state__title">No folders configured</p>
        <p className="state__detail">
          Add at least one entry to the <code>roots</code> array in kbviewer.config.json and restart the server.
        </p>
      </div>
    );
  }

  return <Navigate to={folderRoute(roots[0].id, '')} replace />;
}
