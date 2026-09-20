import { Fragment, Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { fetchDocument } from '../api/client';
import { eventTouches } from '../api/events';
import { Link } from 'react-router-dom';
import { baseName, tagRoute } from '../api/paths';
import type { ChangeEvent, DocumentMeta } from '../api/types';
import { DocumentBody } from '../components/viewers/registry';
import { toggleTask } from '../api/client';
import { Banner, ErrorState, LoadingState } from '../components/ui/States';
import { useFindShortcut } from '../hooks/useFindShortcut';
import { FindContext } from '../components/content/find-context';
import { useAsyncResource } from '../hooks/useAsyncResource';
import { useChangeEvents } from '../hooks/useChangeEvents';
import { formatDateTime, formatSize, kindLabel } from '../lib/format';
import { useFileActions } from '../state/file-actions-context';
import { useVault } from '../state/vault-context';
import { recordOpen } from '../lib/recents';
import { Button } from '../components/ui/Button';

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

/** Frontmatter other than tags, which the tag chips above already show. */
function Frontmatter({ fields }: { fields?: { [key: string]: string } }) {
  const entries = Object.entries(fields ?? {}).filter(([key]) => key !== 'tags');
  if (entries.length === 0) return null;
  return (
    <dl className="doc__frontmatter">
      {entries.map(([key, value]) => (
        <Fragment key={key}>
          <dt>{key}</dt>
          <dd>{value}</dd>
        </Fragment>
      ))}
    </dl>
  );
}

export function DocumentPage({ rootId, path, onTitleChange }: DocumentPageProps) {
  const { canEdit, root } = useVault();
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
  const [findOpen, setFindOpen] = useState(false);
  const find = useMemo(() => ({ open: findOpen, close: () => setFindOpen(false) }), [findOpen]);
  const openFind = useCallback(() => setFindOpen(true), []);
  // The find bar belongs to the rendered prose; leaving it open would resurface it after editing.
  const startEditing = useCallback(() => {
    setFindOpen(false);
    setEditing(true);
  }, []);
  const findable = !editing && Boolean(payload?.html);
  useFindShortcut(findable ? openFind : null);

  // Opening a different document always starts in read mode.
  const documentKey = `${rootId}/${path}`;
  const [openedKey, setOpenedKey] = useState(documentKey);
  if (openedKey !== documentKey) {
    setOpenedKey(documentKey);
    setEditing(false);
    setEditorDirty(false);
    setChangedOnDisk(false);
    setFindOpen(false);
  }

  useEffect(() => {
    onTitleChange(payload?.meta.title || baseName(path));
  }, [payload?.meta.title, path, onTitleChange]);

  // Only a document that actually loaded counts as opened; a 404 is not worth resuming.
  const loadedTitle = payload ? payload.meta.title || baseName(path) : null;
  useEffect(() => {
    if (loadedTitle !== null) recordOpen(rootId, path, loadedTitle);
  }, [rootId, path, loadedTitle]);

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
          {root?.readOnly ? <span className="badge">Read-only</span> : null}
        </p>
        {meta.tags && meta.tags.length > 0 ? (
          <p className="doc__tags">
            {meta.tags.map((tag) => (
              <Link className="tag" key={tag} to={tagRoute(rootId, tag)}>
                #{tag}
              </Link>
            ))}
          </p>
        ) : null}
        <Frontmatter fields={payload.frontmatter} />

        <div className="editor__bar doc__actions">
          {editable ? (
            <Button onClick={startEditing}>
              {/* Decorative: its siblings carry no icon, and announcing "pencil Edit"
                  makes this one button read differently from the rest of the row. */}
              <span aria-hidden="true">✏️</span>
              Edit
            </Button>
          ) : null}
          {canEdit ? (
            <>
              <Button onClick={() => actions.rename(path, false)}>Rename</Button>
              <Button onClick={() => actions.move(path, false)}>Move</Button>
              <Button variant="danger-quiet" onClick={() => actions.remove(path, false)}>
                Delete
              </Button>
            </>
          ) : null}
          {findable ? <Button onClick={openFind}>Find</Button> : null}
          <Button onClick={() => window.print()}>Print</Button>
        </div>

        {payload.renderWarning ? <Banner tone="warning">{payload.renderWarning}</Banner> : null}
        {changedOnDisk ? (
          <Banner tone="info" actions={<Button onClick={reload}>Reload</Button>}>
            This document changed on disk.
          </Banner>
        ) : null}
      </div>

      <FindContext value={find}>
        <DocumentBody payload={payload} rootId={rootId} onToggleTask={editable ? onToggleTask : undefined} />
      </FindContext>
    </article>
  );
}
