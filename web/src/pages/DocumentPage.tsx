import { Suspense, lazy, useCallback, useEffect, useRef, useState } from 'react';
import { fetchDocument } from '../api/client';
import { eventTouches } from '../api/events';
import { baseName } from '../api/paths';
import type { ChangeEvent, DocumentMeta } from '../api/types';
import { DocumentBody } from '../components/viewers/registry';
import { toggleTask } from '../api/client';
import { Banner, ErrorState, LoadingState } from '../components/ui/States';
import { useAsyncResource } from '../hooks/useAsyncResource';
import { useChangeEvents } from '../hooks/useChangeEvents';
import { formatDateTime, formatSize, kindLabel } from '../lib/format';
import { useFileActions } from '../state/file-actions-context';
import { useVault } from '../state/vault-context';

/**
 * CodeMirror and everything under `components/editor` load only when the user
 * actually opens the editor, keeping them out of the initial bundle.
 */
const EditorPane = lazy(() =>
  import('../components/editor/EditorPane').then((module) => ({ default: module.EditorPane })),
);

interface DocumentPageProps {
  rootId: string;
  path: string;
  onTitleChange: (title: string) => void;
}

export function DocumentPage({ rootId, path, onTitleChange }: DocumentPageProps) {
  const { canEdit } = useVault();
  const actions = useFileActions();

  const load = useCallback((signal: AbortSignal) => fetchDocument(rootId, path, signal), [rootId, path]);
  const resource = useAsyncResource(load);
  const { data: payload, setData } = resource;

  // Read inside an async handler, where a captured `payload` would already be stale.
  const payloadRef = useRef(payload);
  useEffect(() => {
    payloadRef.current = payload;
  }, [payload]);

  const [editing, setEditing] = useState(false);
  const [editorDirty, setEditorDirty] = useState(false);
  const [changedOnDisk, setChangedOnDisk] = useState(false);

  // Opening a different document always starts in read mode.
  const documentKey = `${rootId}/${path}`;
  const [openedKey, setOpenedKey] = useState(documentKey);
  if (openedKey !== documentKey) {
    setOpenedKey(documentKey);
    setEditing(false);
    setEditorDirty(false);
    setChangedOnDisk(false);
  }

  useEffect(() => {
    onTitleChange(payload?.meta.title || baseName(path));
  }, [payload?.meta.title, path, onTitleChange]);

  const { reload } = resource;
  const onChange = useCallback(
    (event: ChangeEvent) => {
      if (!eventTouches(event, rootId, path)) return;
      // Never clobber a buffer the user is still typing into.
      if (editing && editorDirty) setChangedOnDisk(true);
      else reload();
    },
    [rootId, path, editing, editorDirty, reload],
  );
  useChangeEvents(onChange);

  /**
   * Ticking a checkbox is a write, so it carries the mtime the page was loaded with, and
   * the returned meta becomes the token for the next tick. A refusal means the document
   * moved underneath the click: reload it and report failure so the box springs back.
   */
  const onToggleTask = useCallback(
    async (line: number, checked: boolean) => {
      const base = payloadRef.current?.meta.mtimeMs;
      if (base === undefined) return false;
      try {
        const meta = await toggleTask(rootId, path, { line, checked, baseMtimeMs: base });
        setData((current) => (current ? { ...current, meta } : current));
        return true;
      } catch {
        reload();
        return false;
      }
    },
    [rootId, path, setData, reload],
  );

  const onSaved = useCallback(
    (meta: DocumentMeta) => {
      setData((current) => (current ? { ...current, meta } : current));
      setChangedOnDisk(false);
      reload();
    },
    [setData, reload],
  );

  if (resource.error) return <ErrorState error={resource.error} onRetry={resource.reload} />;
  if (!payload) return resource.loading ? <LoadingState label="Loading document…" /> : null;

  const { meta } = payload;
  const editable = canEdit && meta.editable;

  if (editing) {
    return (
      <Suspense fallback={<LoadingState label="Loading editor…" />}>
        <EditorPane
          rootId={rootId}
          path={path}
          kind={meta.kind}
          baseMtimeMs={meta.mtimeMs}
          changedOnDisk={changedOnDisk}
          onDiskChangeHandled={() => setChangedOnDisk(false)}
          onSaved={onSaved}
          onDirtyChange={setEditorDirty}
          onClose={() => {
            setEditing(false);
            setChangedOnDisk(false);
            reload();
          }}
        />
      </Suspense>
    );
  }

  return (
    <article className="doc">
      <div className="doc__inner">
        {/* The document's own markdown almost always opens with its title as an `h1`.
            Emitting another here gives every page two `h1`s and a broken outline, so the
            page-chrome title is a `p` styled to look the same. */}
        <p className="doc__title">{meta.title || meta.name}</p>
        <p className="doc__meta">
          <span>{kindLabel(meta.kind)}</span>
          <span>{formatSize(meta.size)}</span>
          <span>Modified {formatDateTime(meta.mtimeMs)}</span>
        </p>
        {meta.tags && meta.tags.length > 0 ? (
          <p className="doc__tags">
            {meta.tags.map((tag) => (
              <span className="tag" key={tag}>
                #{tag}
              </span>
            ))}
          </p>
        ) : null}

        <div className="editor__bar doc__actions">
          {editable ? (
            <button type="button" className="btn" onClick={() => setEditing(true)}>
              {/* Decorative: its siblings carry no icon, and announcing "pencil Edit"
                  makes this one button read differently from the rest of the row. */}
              <span aria-hidden="true">✏️</span>
              Edit
            </button>
          ) : null}
          {canEdit ? (
            <>
              <button type="button" className="btn" onClick={() => actions.rename(path, false)}>
                Rename
              </button>
              <button type="button" className="btn btn--danger-quiet" onClick={() => actions.remove(path, false)}>
                Delete
              </button>
            </>
          ) : null}
        </div>

        {payload.renderWarning ? <Banner tone="warning">{payload.renderWarning}</Banner> : null}
        {changedOnDisk ? (
          <Banner
            tone="info"
            actions={
              <button type="button" className="btn" onClick={reload}>
                Reload
              </button>
            }
          >
            This document changed on disk.
          </Banner>
        ) : null}
      </div>

      <DocumentBody
        payload={payload}
        rootId={rootId}
        onToggleTask={editable ? onToggleTask : undefined}
      />
    </article>
  );
}
