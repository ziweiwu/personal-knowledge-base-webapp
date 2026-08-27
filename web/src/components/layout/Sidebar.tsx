import { useNavigate } from 'react-router-dom';
import { folderRoute } from '../../api/paths';
import { useConnectionState } from '../../hooks/useChangeEvents';
import { useAuth } from '../../state/auth-context';
import { useFileActions } from '../../state/file-actions-context';
import { useRoots } from '../../state/roots-context';
import { useVault } from '../../state/vault-context';
import { TreeView } from '../tree/TreeView';
import { ThemeToggle } from './ThemeToggle';

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
        <select
          id="root-picker"
          className="select"
          value={rootId}
          onChange={(event) => void navigate(folderRoute(event.target.value, ''))}
          style={{ flex: 1, minWidth: 0 }}
        >
          {roots.length === 0 ? <option value={rootId}>{root?.name ?? rootId}</option> : null}
          {roots.map((candidate) => (
            <option key={candidate.id} value={candidate.id}>
              {candidate.name}
            </option>
          ))}
        </select>

        {canEdit ? (
          <>
            <button
              type="button"
              className="btn btn--icon"
              onClick={() => actions.newNote(currentDirectory)}
              aria-label="New note in the current folder"
              title="New note"
            >
              <span aria-hidden="true">✚</span>
            </button>
            <button
              type="button"
              className="btn btn--icon"
              onClick={() => actions.newFolder(currentDirectory)}
              aria-label="New folder in the current folder"
              title="New folder"
            >
              <span aria-hidden="true">📁</span>
            </button>
          </>
        ) : null}
      </div>

      <div className="sidebar__scroll">
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
        <ThemeToggle />
        <button type="button" className="btn btn--ghost" onClick={() => void signOut()}>
          Sign out
        </button>
      </div>
    </>
  );
}
