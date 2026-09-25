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
import { useIsWide } from '../hooks/useMediaQuery';
import { LinkRefs } from '../components/content/LinkRefs';
import { hasTableOfContents } from '../lib/headings';
import { FindContext } from '../components/content/find-context';
import { useAsyncResource } from '../hooks/useAsyncResource';
import { useChangeEvents } from '../hooks/useChangeEvents';
import { formatDateTime, formatSize, kindLabel } from '../lib/format';
import { useFileActions } from '../state/file-actions-context';
import { useVault } from '../state/vault-context';
import { recordOpen } from '../lib/recents';
import { Button } from '../components/ui/Button';
import { ContextMenu, type MenuItem } from '../components/ui/ContextMenu';
import { Icon } from '../components/ui/Icon';

const MENU_OFFSET_PX = 4;

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
  const wide = useIsWide();

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

  // Everything but Edit lives behind one labelled menu, anchored under its button.
  const [menuAnchor, setMenuAnchor] = useState<{ x: number; y: number } | null>(null);
  const openMenu = useCallback((event: React.MouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    setMenuAnchor({ x: rect.left, y: rect.bottom + MENU_OFFSET_PX });
  }, []);
  const closeMenu = useCallback(() => setMenuAnchor(null), []);

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
  const title = meta.title || meta.name;
  const firstHeading = payload.headings[0];
  const bodyCarriesTitle = firstHeading?.depth === 1 && firstHeading.text.trim() === title.trim();
  // Only rendered text has links to list; a PDF or an image has neither.
  const linksListed = meta.kind === 'markdown' || meta.kind === 'docx';
  // Wide enough, the tags step out of the header into the right margin, beside the links.
  const tags =
    meta.tags && meta.tags.length > 0 ? (
      <p className="doc__tags">
        {meta.tags.map((tag) => (
          <Link className="tag" key={tag} to={tagRoute(rootId, tag)}>
            #{tag}
          </Link>
        ))}
      </p>
    ) : null;
  const tagsInMargin = wide ? tags : null;
  const docClass = ['doc', 'doc--reading', hasTableOfContents(payload.headings) && 'doc--with-toc']
    .filter(Boolean)
    .join(' ');

  const menuItems: MenuItem[] = [];
  if (canEdit) {
    menuItems.push({ id: 'rename', label: 'Rename', icon: 'rename', onSelect: () => actions.rename(path, false) });
    menuItems.push({ id: 'move', label: 'Move', icon: 'folder', onSelect: () => actions.move(path, false) });
  }
  if (findable) menuItems.push({ id: 'find', label: 'Find', icon: 'search', onSelect: openFind });
  menuItems.push({ id: 'print', label: 'Print', icon: 'print', onSelect: () => window.print() });
  if (canEdit) {
    menuItems.push({
      id: 'delete',
      label: 'Delete',
      icon: 'trash',
      danger: true,
      separatorBefore: true,
      onSelect: () => actions.remove(path, false),
    });
  }

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
    <article className={docClass}>
      <header className="doc__inner doc__head">
        {/* The document's own markdown almost always opens with its title as an `h1`.
            Emitting another here gives every page two `h1`s and a broken outline, so the
            page-chrome title is a `p`. When the note's first heading already is the
            title, the `p` steps down to a caption so the words are set large once. */}
        <p className={bodyCarriesTitle ? 'doc__title doc__title--caption' : 'doc__title'}>{title}</p>
        <p className="doc__meta">
          <span>{kindLabel(meta.kind)}</span>
          <span>{formatSize(meta.size)}</span>
          <span>Modified {formatDateTime(meta.mtimeMs)}</span>
          {root?.readOnly ? <span className="badge">Read-only</span> : null}
        </p>
        {wide ? null : tags}
        <Frontmatter fields={payload.frontmatter} />

        <div className="editor__bar doc__actions">
          {editable ? (
            <Button variant="primary" onClick={startEditing}>
              <Icon name="edit" />
              Edit
            </Button>
          ) : null}
          <Button aria-haspopup="menu" aria-expanded={menuAnchor !== null} onClick={openMenu}>
            <Icon name="more" />
            More
          </Button>
        </div>
        {menuAnchor ? (
          <ContextMenu items={menuItems} anchor={menuAnchor} label="Document actions" onClose={closeMenu} />
        ) : null}

        {payload.renderWarning ? <Banner tone="warning">{payload.renderWarning}</Banner> : null}
        {changedOnDisk ? (
          <Banner tone="info" actions={<Button onClick={reload}>Reload</Button>}>
            This document changed on disk.
          </Banner>
        ) : null}
      </header>

      <FindContext value={find}>
        <DocumentBody payload={payload} rootId={rootId} onToggleTask={editable ? onToggleTask : undefined} />
      </FindContext>

      {tagsInMargin || linksListed ? (
        <aside className="doc__inner doc__margin" aria-label="Tags and links">
          {tagsInMargin}
          {linksListed ? (
            <>
              <LinkRefs title="Backlinks" refs={payload.backlinks} rootId={rootId} emptyLabel="No other document links here yet." />
              <LinkRefs title="Links from this note" refs={payload.outlinks} rootId={rootId} />
            </>
          ) : null}
        </aside>
      ) : null}
    </article>
  );
}
