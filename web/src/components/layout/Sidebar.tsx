import { useNavigate } from 'react-router-dom';
import { folderRoute, trashRoute } from '../../api/paths';
import { useConnectionState } from '../../hooks/useChangeEvents';
import { useAuth } from '../../state/auth-context';
import { useFileActions } from '../../state/file-actions-context';
import { useRoots } from '../../state/roots-context';
import { useVault } from '../../state/vault-context';
import { lastRoute } from '../../lib/recents';
import { RecentList } from '../home/RecentList';
import { TreeView } from '../tree/TreeView';
import { Button } from '../ui/Button';
import { Select } from '../ui/Select';
import { Icon } from '../ui/Icon';

/** Pinned notes plus a few recents; the tree below is the full list. */
const SIDEBAR_RECENTS_LIMIT = 6;

const CONNECTION_LABELS = {
  connecting: 'Connecting to live updates',
  live: 'Live updates connected',
  down: 'Live updates disconnected',
} as const;

interface SidebarProps {
  activePath: string;
  /** Current folder for "new here" actions: the folder itself, or a document's parent. */
  currentDirectory: string;
  onNavigate?: () => void;
}

export function Sidebar({ activePath, currentDirectory, onNavigate }: SidebarProps) {
  const { roots } = useRoots();
  const { rootId, root, canEdit } = useVault();
  const actions = useFileActions();
  const { session, signOut } = useAuth();
  const connection = useConnectionState();
  const navigate = useNavigate();

  return (
    <>
      <div className="sidebar__head">
        <label className="sr-only" htmlFor="root-picker">
          Folder collection
        </label>
        <Select
          id="root-picker"
          grow
          value={rootId}
          // Switching back to a root lands where the user left it, not on its listing.
          onChange={(event) => void navigate(lastRoute(event.target.value) ?? folderRoute(event.target.value, ''))}
        >
          {roots.length === 0 ? <option value={rootId}>{root?.name ?? rootId}</option> : null}
          {roots.map((candidate) => (
            <option key={candidate.id} value={candidate.id}>
              {candidate.name}
            </option>
          ))}
        </Select>

        {canEdit ? (
          <>
            <Button
              variant="icon"
              onClick={() => actions.newNote(currentDirectory)}
              aria-label="New note in the current folder"
              title="New note"
            >
              <Icon name="file-plus" size="md" />
            </Button>
            <Button
              variant="icon"
              onClick={() => actions.newFolder(currentDirectory)}
              aria-label="New folder in the current folder"
              title="New folder"
            >
              <Icon name="folder-plus" size="md" />
            </Button>
            <Button
              variant="icon"
              onClick={() => void navigate(trashRoute(rootId))}
              aria-label="Open the trash for this collection"
              title="Trash"
            >
              <Icon name="trash" size="md" />
            </Button>
          </>
        ) : null}
      </div>

      <div className="sidebar__scroll">
        <RecentList
          rootId={rootId}
          activePath={activePath}
          onNavigate={onNavigate}
          headingId="sidebar-recents"
          heading="Recent"
          limit={SIDEBAR_RECENTS_LIMIT}
        />
        <TreeView key={rootId} activePath={activePath} onNavigate={onNavigate} />
      </div>

      <div className="sidebar__foot">
        <span
          className={`conn-dot conn-dot--${connection === 'live' ? 'live' : connection === 'down' ? 'down' : 'idle'}`}
          role="img"
          aria-label={CONNECTION_LABELS[connection]}
          title={CONNECTION_LABELS[connection]}
        />
        <span className="sidebar__email">{session?.email ?? ''}</span>
        <Button variant="ghost" onClick={() => void signOut()}>
          Sign out
        </Button>
      </div>
    </>
  );
}
