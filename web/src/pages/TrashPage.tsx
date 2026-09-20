import { useCallback, useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { fetchTrash, restoreFromTrash } from '../api/client';
import { folderRoute, parentPath } from '../api/paths';
import type { TrashEntry } from '../api/types';
import { Button } from '../components/ui/Button';
import { EmptyState, ErrorState, LoadingState } from '../components/ui/States';
import { useAsyncResource } from '../hooks/useAsyncResource';
import { describeError } from '../lib/errors';
import { formatDateTime, formatSize } from '../lib/format';
import { useRoots } from '../state/roots-context';
import { useToast } from '../state/toast-context';
import { Icon } from '../components/ui/Icon';

interface RowProps {
  entry: TrashEntry;
  rootId: string;
  busy: boolean;
  onRestore: (entry: TrashEntry) => void;
}

function TrashRow({ entry, rootId, busy, onRestore }: RowProps) {
  const folder = parentPath(entry.originalPath);
  return (
    <li className="trash__row">
      <div className="trash__body">
        <span className="trash__name">{entry.name}</span>
        <span className="trash__meta">
          {folder ? (
            <>
              from <Link to={folderRoute(rootId, folder)}>{folder}/</Link> ·{' '}
            </>
          ) : null}
          {formatSize(entry.size)} · last edited {formatDateTime(entry.mtimeMs)}
        </span>
      </div>
      <Button onClick={() => onRestore(entry)} disabled={busy} aria-label={`Restore ${entry.originalPath}`}>
        {busy ? 'Restoring…' : 'Restore'}
      </Button>
    </li>
  );
}

/** Restore one entry, reporting the outcome as a toast; the caller reloads the list. */
function useRestore(rootId: string, reload: () => void) {
  const toast = useToast();
  const [restoring, setRestoring] = useState<string | null>(null);
  const restore = async (entry: TrashEntry) => {
    setRestoring(entry.trashPath);
    try {
      await restoreFromTrash(rootId, entry.trashPath);
      toast.show(`Restored ${entry.originalPath}`);
      reload();
    } catch (cause) {
      const { detail } = describeError(cause instanceof Error ? cause : new Error(String(cause)));
      toast.show(`Could not restore ${entry.name}: ${detail}`, 'danger');
    } finally {
      setRestoring(null);
    }
  };
  return { restoring, restore };
}

function TrashHeader({ rootId, rootName }: { rootId: string; rootName: string }) {
  return (
    <header className="trash__head">
      <Link to={folderRoute(rootId, '')} className="trash__back">
        <Icon name="arrow-left" /> {rootName}
      </Link>
      <h1 className="trash__title">Trash</h1>
      <p className="trash__lede">
        Deleted notes wait here in this collection&apos;s <code>.trash/</code> folder until you restore them or remove
        them on disk.
      </p>
    </header>
  );
}

/**
 * What delete moved aside for one collection. Restore is the only verb: emptying the
 * trash for good stays a deliberate act on disk, since this page exists to undo mistakes,
 * not to make a new one possible from a phone.
 */
export function TrashPage() {
  const { rootId = '' } = useParams();
  const { roots } = useRoots();
  const root = roots.find((candidate) => candidate.id === rootId) ?? null;
  const rootName = root?.name ?? rootId;

  const load = useCallback((signal: AbortSignal) => fetchTrash(rootId, signal), [rootId]);
  const trash = useAsyncResource(load);
  const { restoring, restore } = useRestore(rootId, trash.reload);

  useEffect(() => {
    document.title = `Trash · ${rootName} · kbviewer`;
  }, [rootName]);

  return (
    <div className="trash">
      <TrashHeader rootId={rootId} rootName={rootName} />
      {root?.readOnly ? (
        <EmptyState
          tone="info"
          title="This collection is read-only"
          detail="Nothing can be deleted from it, so its trash stays empty."
        >
          <Link className="btn" to={folderRoute(rootId, '')}>
            Back to the collection
          </Link>
        </EmptyState>
      ) : (
        <TrashList trash={trash} rootId={rootId} restoring={restoring} onRestore={(entry) => void restore(entry)} />
      )}
    </div>
  );
}

interface ListProps {
  trash: ReturnType<typeof useAsyncResource<TrashEntry[]>>;
  rootId: string;
  restoring: string | null;
  onRestore: (entry: TrashEntry) => void;
}

function TrashList({ trash, rootId, restoring, onRestore }: ListProps) {
  if (trash.error) return <ErrorState error={trash.error} onRetry={trash.reload} />;
  if (!trash.data) return <LoadingState label="Loading the trash…" />;
  if (trash.data.length === 0) {
    return <EmptyState title="Trash is empty" detail="Anything you delete from this collection will appear here." />;
  }
  return (
    <ul className="trash__list" aria-label="Deleted files">
      {trash.data.map((entry) => (
        <TrashRow
          key={entry.trashPath}
          entry={entry}
          rootId={rootId}
          busy={restoring === entry.trashPath}
          onRestore={onRestore}
        />
      ))}
    </ul>
  );
}
