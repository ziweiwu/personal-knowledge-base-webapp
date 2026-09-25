import { Link } from 'react-router-dom';
import { docRoute } from '../../api/paths';
import { isPinned, listRecents, togglePin, useRecents, type RecentNote } from '../../lib/recents';
import { Button } from '../ui/Button';
import { ICON_FILLED, Icon } from '../ui/Icon';

interface RecentListProps {
  /** Limit to one root (the sidebar) or show every root (the home page). */
  rootId?: string;
  activePath?: string;
  /** Names a note's root when the list spans several; also drops roots no longer configured. */
  rootNames?: Map<string, string>;
  /** One list of everything (the sidebar), or the pinned and the merely recent apart (the home page). */
  only?: 'pinned' | 'unpinned';
  onNavigate?: () => void;
  headingId: string;
  heading: string;
  /** Entries beyond this stay reachable from the tree, so the list stays short. */
  limit: number;
}

function PinToggle({ note }: { note: RecentNote }) {
  const pinned = isPinned(note.rootId, note.path);
  return (
    <Button
      variant="icon"
      className="recents__pin"
      aria-pressed={pinned}
      aria-label={pinned ? `Unpin ${note.title}` : `Pin ${note.title}`}
      title={pinned ? 'Unpin' : 'Pin'}
      onClick={() => togglePin(note)}
    >
      <Icon name="pin" className={pinned ? ICON_FILLED : undefined} />
    </Button>
  );
}

interface RecentRowProps {
  note: RecentNote;
  active: boolean;
  rootName?: string;
  onNavigate?: () => void;
}

function RecentRow({ note, active, rootName, onNavigate }: RecentRowProps) {
  return (
    <li className={`recents__item${active ? ' recents__item--selected' : ''}`}>
      <Link
        className="recents__link"
        to={docRoute(note.rootId, note.path)}
        onClick={onNavigate}
        aria-current={active ? 'page' : undefined}
      >
        <span className="recents__title">{note.title}</span>
        {rootName ? <span className="recents__root">{rootName}</span> : null}
      </Link>
      <PinToggle note={note} />
    </li>
  );
}

export function RecentList({
  rootId,
  activePath,
  rootNames,
  only,
  onNavigate,
  headingId,
  heading,
  limit,
}: RecentListProps) {
  const snapshot = useRecents();
  // A root that was removed from the config has nothing to link to.
  const items = listRecents(snapshot, rootId)
    .filter((note) => !rootNames || rootNames.has(note.rootId))
    .filter((note) => only === undefined || (only === 'pinned') === isPinned(note.rootId, note.path))
    .slice(0, limit);
  if (items.length === 0) return null;

  return (
    <section className="recents" aria-labelledby={headingId}>
      <h2 className="recents__heading" id={headingId}>
        {heading}
      </h2>
      <ul className="recents__list">
        {items.map((note) => (
          <RecentRow
            key={`${note.rootId}/${note.path}`}
            note={note}
            active={rootId !== undefined && note.path === activePath}
            rootName={rootNames?.get(note.rootId)}
            onNavigate={onNavigate}
          />
        ))}
      </ul>
    </section>
  );
}
