import { useCallback, useEffect, useRef, useState } from 'react';
import { SaveConflictError, fetchDocument, fetchRaw, saveDocument } from '../../api/client';
import type { DocumentMeta, SaveConflict } from '../../api/types';
import { useAsyncResource } from '../../hooks/useAsyncResource';
import { formatDateTime } from '../../lib/format';
import { useTheme } from '../../state/theme-context';
import { Banner, ErrorState, LoadingState } from '../ui/States';
import { describeError } from '../../lib/errors';
import { CodeMirrorField, type EditorHandle } from './CodeMirrorField';
import { ConflictDialog } from './ConflictDialog';
import { FormatToolbar } from './FormatToolbar';
import { Button } from '../ui/Button';
import { AUTOSAVE_IDLE_SECONDS, useAutosave } from './useAutosave';

export interface EditorPaneProps {
  rootId: string;
  path: string;
  kind: string;
  baseMtimeMs: number;
  /** True once a change event for this file arrived while the buffer was dirty. */
  changedOnDisk: boolean;
  onDiskChangeHandled: () => void;
  onSaved: (meta: DocumentMeta) => void;
  onDirtyChange: (dirty: boolean) => void;
  onClose: () => void;
}

export function EditorPane({
  rootId,
  path,
  kind,
  baseMtimeMs,
  changedOnDisk,
  onDiskChangeHandled,
  onSaved,
  onDirtyChange,
  onClose,
}: EditorPaneProps) {
  const { resolved: theme } = useTheme();
  const load = useCallback((signal: AbortSignal) => fetchRaw(rootId, path, signal), [rootId, path]);
  const source = useAsyncResource(load);

  const handleRef = useRef<EditorHandle | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [conflict, setConflict] = useState<SaveConflict | null>(null);
  const [base, setBase] = useState(baseMtimeMs);
  const [editCount, setEditCount] = useState(0);
  // A failed save is not retried on a timer; the next keystroke re-arms the autosave.
  const [autosavePaused, setAutosavePaused] = useState(false);

  useEffect(() => onDirtyChange(dirty), [dirty, onDirtyChange]);

  // Read inside async work, where the `dirty` captured by a closure may already be stale.
  const dirtyRef = useRef(dirty);
  useEffect(() => {
    dirtyRef.current = dirty;
  }, [dirty]);

  /**
   * The file on disk is no longer the revision this buffer was loaded from.
   *
   * `base` is the concurrency token sent with every save, and it may only ever advance
   * *together with* the buffer it describes. Advancing it on its own would tell the
   * server "I am editing the current revision" while the buffer still held older text,
   * which defeats the 409 check and silently discards whoever wrote the file in between.
   */
  const diskMovedAway = baseMtimeMs !== base;

  useEffect(() => {
    if (!diskMovedAway) return;
    // A dirty buffer is never replaced. `base` is deliberately left behind so that if the
    // user saves anyway, the server rejects it with a conflict instead of overwriting.
    if (dirty) return;

    let cancelled = false;
    void (async () => {
      try {
        const text = await fetchRaw(rootId, path);
        // The user may have started typing while this was in flight.
        if (cancelled || dirtyRef.current) return;
        handleRef.current?.setValue(text);
        setDirty(false);
        setBase(baseMtimeMs);
        setSavedAt(null);
      } catch {
        // Leave `base` behind on failure: a stale token costs a conflict dialog, whereas
        // advancing it without the text costs the other writer their work.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [diskMovedAway, dirty, baseMtimeMs, rootId, path]);

  // An accidental tab close should not throw away an unsaved buffer.
  useEffect(() => {
    if (!dirty) return;
    const warn = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener('beforeunload', warn);
    return () => window.removeEventListener('beforeunload', warn);
  }, [dirty]);

  const applySaved = useCallback(
    (meta: DocumentMeta) => {
      setBase(meta.mtimeMs);
      setDirty(false);
      setSavedAt(meta.mtimeMs);
      setSaveError(null);
      onSaved(meta);
    },
    [onSaved],
  );

  const save = useCallback(async () => {
    const content = handleRef.current?.getValue();
    if (content === undefined) return;
    setSaving(true);
    setSaveError(null);
    try {
      applySaved(await saveDocument(rootId, path, { content, baseMtimeMs: base }));
    } catch (cause) {
      setAutosavePaused(true);
      if (cause instanceof SaveConflictError) setConflict(cause.conflict);
      else setSaveError(cause instanceof Error ? describeError(cause).detail : String(cause));
    } finally {
      setSaving(false);
    }
  }, [applySaved, base, path, rootId]);

  const autosaveBlocked = !dirty || saving || autosavePaused || conflict !== null || diskMovedAway;
  useAutosave({ save: autosaveBlocked ? null : save, editCount });

  const onEdit = () => {
    setDirty(true);
    setAutosavePaused(false);
    setEditCount((count) => count + 1);
  };

  /** "Keep mine": re-save the buffer against the disk mtime we were just handed. */
  const keepMine = async () => {
    if (!conflict) return;
    setSaving(true);
    setSaveError(null);
    try {
      applySaved(
        await saveDocument(rootId, path, { content: conflict.yourContent, baseMtimeMs: conflict.diskMtimeMs }),
      );
      setConflict(null);
    } catch (cause) {
      if (cause instanceof SaveConflictError) setConflict(cause.conflict);
      else setSaveError(cause instanceof Error ? describeError(cause).detail : String(cause));
    } finally {
      setSaving(false);
    }
  };

  /** "Take theirs": adopt the on-disk text without writing anything. */
  const takeTheirs = () => {
    if (!conflict) return;
    handleRef.current?.setValue(conflict.diskContent);
    setBase(conflict.diskMtimeMs);
    setDirty(false);
    setSavedAt(null);
    setConflict(null);
    setSaveError(null);
    onDiskChangeHandled();
  };

  const reloadFromDisk = async () => {
    try {
      const [payload, text] = await Promise.all([fetchDocument(rootId, path), fetchRaw(rootId, path)]);
      handleRef.current?.setValue(text);
      setBase(payload.meta.mtimeMs);
      setDirty(false);
      setSavedAt(null);
      setSaveError(null);
      onDiskChangeHandled();
    } catch (cause) {
      setSaveError(cause instanceof Error ? describeError(cause).detail : String(cause));
    }
  };

  const requestClose = () => {
    if (dirty && !window.confirm('Discard unsaved changes?')) return;
    onClose();
  };

  if (source.error) return <ErrorState error={source.error} onRetry={source.reload} />;
  if (source.data === null) return source.loading ? <LoadingState label="Loading source…" /> : null;

  return (
    <div className="editor">
      <div className="editor__bar">
        <Button variant="primary" onClick={() => void save()} disabled={saving || !dirty}>
          {saving ? 'Saving…' : 'Save'}
        </Button>
        <Button onClick={requestClose} disabled={saving}>
          Done
        </Button>
        <span className="editor__status" role="status">
          {saving
            ? 'Saving…'
            : dirty
              ? 'Unsaved changes'
              : savedAt
                ? `Saved ${formatDateTime(savedAt)}`
                : `Last modified ${formatDateTime(base)}`}
        </span>
        <span className="editor__status editor__status--fixed only-desktop">
          <kbd>⌘S</kbd> to save
        </span>
        <span className="editor__status editor__status--fixed">
          Autosaves {AUTOSAVE_IDLE_SECONDS} s after you stop typing
        </span>
        {kind === 'markdown' ? <FormatToolbar onRun={(command) => handleRef.current?.run(command)} /> : null}
      </div>

      {/* Suppressed while the conflict modal is open: both describe the same change,
          with two different action vocabularies for one decision. */}
      {!conflict && (changedOnDisk || (diskMovedAway && dirty)) ? (
        <Banner
          tone="warning"
          actions={
            <>
              <Button onClick={() => void reloadFromDisk()}>Reload from disk</Button>
              <Button variant="ghost" onClick={onDiskChangeHandled}>
                Keep editing
              </Button>
            </>
          }
        >
          This file changed on disk while you were editing. Your unsaved text has been left alone.
        </Banner>
      ) : null}

      {saveError ? <Banner tone="danger">{saveError}</Banner> : null}

      <CodeMirrorField
        initialValue={source.data}
        rootId={rootId}
        language={kind === 'markdown' ? 'markdown' : 'plain'}
        theme={theme}
        onChange={onEdit}
        onSave={() => void save()}
        onReady={(handle) => {
          handleRef.current = handle;
        }}
      />

      {conflict ? (
        <ConflictDialog
          conflict={conflict}
          busy={saving}
          error={saveError}
          onKeepMine={() => void keepMine()}
          onTakeTheirs={takeTheirs}
          onCancel={() => setConflict(null)}
        />
      ) : null}
    </div>
  );
}
