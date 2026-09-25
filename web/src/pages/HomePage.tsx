import { Link } from 'react-router-dom';
import { folderRoute } from '../api/paths';
import type { RootInfo } from '../api/types';
import { RecentList } from '../components/home/RecentList';
import { ThemeToggle } from '../components/layout/ThemeToggle';
import { Button } from '../components/ui/Button';
import { EmptyState, ErrorState, LoadingState } from '../components/ui/States';
import { formatRelative } from '../lib/format';
import { lastRoute } from '../lib/recents';
import { useAuth } from '../state/auth-context';
import { useRoots } from '../state/roots-context';

/** Enough to resume yesterday's work; anything older is one search away. */
const HOME_RECENTS_LIMIT = 10;

/** A collection is one line of the contents page: its name, a leader, and its size. */
function RootRow({ root }: { root: RootInfo }) {
  return (
    <Link className="root-card" to={lastRoute(root.id) ?? folderRoute(root.id, '')}>
      <span className="root-card__name">{root.name}</span>
      <span className="root-card__leader" aria-hidden="true" />
      <span className="root-card__meta">
        <span>{root.documents === 1 ? '1 document' : `${root.documents} documents`}</span>
        {root.lastModifiedMs !== null ? <span>Updated {formatRelative(root.lastModifiedMs)}</span> : null}
        {root.readOnly ? <span className="badge">Read-only</span> : null}
      </span>
    </Link>
  );
}

function NoRoots() {
  return (
    <EmptyState
      tone="info"
      title="No folders configured"
      detail={
        <>
          Add at least one entry to the <code>roots</code> array in kbviewer.config.json and restart the server.
        </>
      }
    />
  );
}

function HomeHeader() {
  const { session, signOut } = useAuth();
  return (
    <header className="home__head">
      <h1 className="home__brand">kbviewer</h1>
      <span className="sidebar__email">{session?.email ?? ''}</span>
      <ThemeToggle />
      <Button variant="ghost" onClick={() => void signOut()}>
        Sign out
      </Button>
    </header>
  );
}

/**
 * The landing page, set like a book's front matter: the collections as a table of
 * contents, then the pinned notes and the ones opened most recently as short indexes.
 */
export function HomePage() {
  const { roots, loading, error, reload } = useRoots();

  if (error) return <ErrorState error={error} onRetry={reload} />;
  if (loading) return <LoadingState label="Loading your collections…" />;

  const rootNames = new Map(roots.map((root) => [root.id, root.name]));

  return (
    <div className="home">
      <HomeHeader />
      <main className="home__main" id="main-content">
        {roots.length === 0 ? (
          <NoRoots />
        ) : (
          <section aria-labelledby="home-roots">
            <h2 className="home__section-title" id="home-roots">
              Collections
            </h2>
            <ul className="home__roots">
              {roots.map((root) => (
                <li key={root.id}>
                  <RootRow root={root} />
                </li>
              ))}
            </ul>
          </section>
        )}
        <RecentList rootNames={rootNames} only="pinned" headingId="home-pins" heading="Pinned" limit={HOME_RECENTS_LIMIT} />
        <RecentList
          rootNames={rootNames}
          only="unpinned"
          headingId="home-recents"
          heading="Recently opened"
          limit={HOME_RECENTS_LIMIT}
        />
      </main>
    </div>
  );
}
