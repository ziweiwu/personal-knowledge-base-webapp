import { useCallback, useMemo, useRef, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import { createDocument, createFolder, deleteDocument, renameDocument, uploadFile } from '../../api/client';
import { baseName, docRoute, folderRoute, joinPath, parentPath } from '../../api/paths';
import { FileActionsContext } from '../../state/file-actions-context';
import { useVault } from '../../state/vault-context';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { PromptDialog } from '../ui/PromptDialog';
import { Banner } from '../ui/States';
import { describeError } from '../../lib/errors';

type Pending =
  | { kind: 'new-note'; directory: string }
  | { kind: 'new-folder'; directory: string }
  | { kind: 'rename'; path: string; isDir: boolean }
  | { kind: 'delete'; path: string; isDir: boolean };

/** A typed name already carries its own extension; anything else gets `.md`. */
const HAS_EXTENSION = /\.[a-z0-9]+$/i;

function errorMessage(cause: unknown): string {
  if (cause instanceof Error) return describeError(cause).detail;
  return String(cause);
}

function fileCount(count: number): string {
  return `${count} ${count === 1 ? 'file' : 'files'}`;
}

/** States what actually landed, so a partial failure is never read as a success. */
function uploadSummary(uploaded: number, attempted: number, destination: string, failures: string[]): string {
  if (failures.length === 0) return `Uploaded ${fileCount(uploaded)} to ${destination}.`;
  if (uploaded === 0) return `Nothing was uploaded — ${failures.join('; ')}`;
  return `Uploaded ${uploaded} of ${fileCount(attempted)} to ${destination}. Failed — ${failures.join('; ')}`;
}

/**
 * Owns every mutating file operation and the dialogs that drive them, so the
 * tree, the folder listing and the toolbar all share one implementation.
 */
export function FileActionsProvider({ children }: { children: ReactNode }) {
  const { rootId, reloadTree, notifyLocalChange } = useVault();
  const navigate = useNavigate();

  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [uploadDirectory, setUploadDirectory] = useState<string>('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  const start = useCallback((next: Pending) => {
    setError(null);
    setPending(next);
  }, []);

  const actions = useMemo(
    () => ({
      newNote: (directory: string) => start({ kind: 'new-note', directory }),
      newFolder: (directory: string) => start({ kind: 'new-folder', directory }),
      rename: (path: string, isDir: boolean) => start({ kind: 'rename', path, isDir }),
      remove: (path: string, isDir: boolean) => start({ kind: 'delete', path, isDir }),
      upload: (directory: string) => {
        setError(null);
        setUploadDirectory(directory);
        fileInputRef.current?.click();
      },
    }),
    [start],
  );

  const finish = useCallback(
    (message: string | null) => {
      setPending(null);
      setBusy(false);
      setNotice(message);
      reloadTree();
      notifyLocalChange();
    },
    [reloadTree, notifyLocalChange],
  );

  const runCreateNote = async (name: string) => {
    if (pending?.kind !== 'new-note') return;
    const fileName = HAS_EXTENSION.test(name) ? name : `${name}.md`;
    const path = joinPath(pending.directory, fileName);
    setBusy(true);
    try {
      await createDocument(rootId, path, `# ${fileName.replace(/\.[^.]+$/, '')}\n\n`);
      finish(`Created ${path}`);
      void navigate(docRoute(rootId, path));
    } catch (cause) {
      setBusy(false);
      setError(errorMessage(cause));
    }
  };

  const runCreateFolder = async (name: string) => {
    if (pending?.kind !== 'new-folder') return;
    const path = joinPath(pending.directory, name);
    setBusy(true);
    try {
      await createFolder(rootId, path);
      finish(`Created folder ${path}`);
      void navigate(folderRoute(rootId, path));
    } catch (cause) {
      setBusy(false);
      setError(errorMessage(cause));
    }
  };

  const runRename = async (name: string) => {
    if (pending?.kind !== 'rename') return;
    const target = joinPath(parentPath(pending.path), name);
    if (target === pending.path) {
      setPending(null);
      return;
    }
    setBusy(true);
    try {
      const result = await renameDocument(rootId, { from: pending.path, to: target, updateLinks: true });
      const updatedCount = result.updated.length;
      finish(
        updatedCount > 0
          ? `Renamed to ${result.to} — also updated links in ${updatedCount} ${updatedCount === 1 ? 'document' : 'documents'}.`
          : `Renamed to ${result.to}.`,
      );
      void navigate(pending.isDir ? folderRoute(rootId, result.to) : docRoute(rootId, result.to));
    } catch (cause) {
      setBusy(false);
      setError(errorMessage(cause));
    }
  };

  const runDelete = async () => {
    if (pending?.kind !== 'delete') return;
    setBusy(true);
    try {
      await deleteDocument(rootId, pending.path);
      finish(`Moved ${baseName(pending.path)} to .trash/`);
      void navigate(folderRoute(rootId, parentPath(pending.path)));
    } catch (cause) {
      setBusy(false);
      setError(errorMessage(cause));
    }
  };

  const runUpload = async (files: FileList) => {
    // The picker is reset as soon as this returns, which empties the live
    // FileList: the count has to be taken from a snapshot, not read back later.
    const queued = Array.from(files);
    const destination = uploadDirectory || 'the root folder';
    setBusy(true);
    const failures: string[] = [];
    let uploaded = 0;
    for (const file of queued) {
      try {
        await uploadFile(rootId, joinPath(uploadDirectory, file.name), file);
        uploaded += 1;
      } catch (cause) {
        failures.push(`${file.name}: ${errorMessage(cause)}`);
      }
    }
    setBusy(false);
    reloadTree();
    notifyLocalChange();
    setNotice(uploadSummary(uploaded, queued.length, destination, failures));
  };

  return (
    <FileActionsContext value={actions}>
      {notice ? (
        <div style={{ position: 'fixed', left: 12, right: 12, bottom: 12, zIndex: 85, maxWidth: 520, margin: '0 auto' }}>
          <Banner tone="info" onDismiss={() => setNotice(null)}>
            {notice}
          </Banner>
        </div>
      ) : null}

      <input
        ref={fileInputRef}
        type="file"
        multiple
        className="sr-only"
        aria-hidden="true"
        tabIndex={-1}
        onChange={(event) => {
          const { files } = event.target;
          if (files && files.length > 0) void runUpload(files);
          event.target.value = '';
        }}
      />

      {pending?.kind === 'new-note' ? (
        <PromptDialog
          title="New note"
          label="File name"
          hint={`Created in ${pending.directory || 'the root folder'}. “.md” is added if you leave the extension off.`}
          submitLabel="Create"
          busy={busy}
          error={error}
          onSubmit={(value) => void runCreateNote(value)}
          onCancel={() => setPending(null)}
        />
      ) : null}

      {pending?.kind === 'new-folder' ? (
        <PromptDialog
          title="New folder"
          label="Folder name"
          hint={`Created in ${pending.directory || 'the root folder'}.`}
          submitLabel="Create"
          busy={busy}
          error={error}
          onSubmit={(value) => void runCreateFolder(value)}
          onCancel={() => setPending(null)}
        />
      ) : null}

      {pending?.kind === 'rename' ? (
        <PromptDialog
          title={pending.isDir ? 'Rename folder' : 'Rename note'}
          label="New name"
          initialValue={baseName(pending.path)}
          hint="Links pointing at this file will be rewritten."
          submitLabel="Rename"
          preselect={pending.isDir ? 'all' : 'stem'}
          busy={busy}
          error={error}
          onSubmit={(value) => void runRename(value)}
          onCancel={() => setPending(null)}
        />
      ) : null}

      {pending?.kind === 'delete' ? (
        <ConfirmDialog
          title={pending.isDir ? 'Delete folder?' : 'Delete note?'}
          message={`“${baseName(pending.path)}” will be moved to .trash/ inside this folder.`}
          detail="Nothing is erased — you can restore it from .trash/ on disk."
          confirmLabel="Move to .trash/"
          danger
          busy={busy}
          error={error}
          onConfirm={() => void runDelete()}
          onCancel={() => setPending(null)}
        />
      ) : null}

      {children}
    </FileActionsContext>
  );
}
