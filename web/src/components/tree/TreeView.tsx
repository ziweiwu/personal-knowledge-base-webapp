import { useCallback, useEffect, useMemo, useState, type KeyboardEvent } from 'react';
import { Link } from 'react-router-dom';
import { docRoute, folderRoute } from '../../api/paths';
import type { TreeNode } from '../../api/types';
import { kindIconName, kindLabel } from '../../lib/format';
import { useFileActions } from '../../state/file-actions-context';
import { useVault } from '../../state/vault-context';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { EmptyState, ErrorState, LoadingState } from '../ui/States';
import { Icon } from '../ui/Icon';

/** One step of tree indentation, matched to the row padding in app.css. */
const INDENT_PER_DEPTH_PX = 12;
/** Gap between the row button and the menu that drops out of it. */
const MENU_OFFSET_PX = 4;

function storageKey(rootId: string): string {
  return `kbviewer.expanded.${rootId}`;
}

function readExpanded(rootId: string): Set<string> {
  try {
    const raw = localStorage.getItem(storageKey(rootId));
    if (raw) return new Set(JSON.parse(raw) as string[]);
  } catch {
    // Expansion state is a convenience; losing it is not worth an error.
  }
  return new Set();
}

function ancestorsOf(path: string): string[] {
  const segments = path.split('/').filter(Boolean).slice(0, -1);
  const result: string[] = [];
  let current = '';
  for (const segment of segments) {
    current = current ? `${current}/${segment}` : segment;
    result.push(current);
  }
  return result;
}

/** What the keyboard asked for on a row: move focus to another row, or open/close this one. */
type TreeKeyAction = { focus: number } | { open: boolean } | null;

/**
 * The rendered rows, in document order, which is also visual order since a collapsed
 * folder renders no children. Reading the DOM instead of the model keeps this in step
 * with sorting and expansion for free.
 */
function treeRows(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>('[data-tree-row]'));
}

function rowDepth(row: HTMLElement): number {
  return Number(row.dataset.depth);
}

function nearestAncestorRow(rows: HTMLElement[], index: number): number | null {
  const depth = rowDepth(rows[index]);
  for (let candidate = index - 1; candidate >= 0; candidate -= 1) {
    if (rowDepth(rows[candidate]) < depth) return candidate;
  }
  return null;
}

/**
 * Up and down walk the visible rows; right opens a folder or steps into it; left closes a
 * folder or steps out to its parent. Home and End match the context menu. A leaf has no
 * children, so right does nothing there rather than jumping to an unrelated sibling.
 */
function resolveTreeKey(key: string, rows: HTMLElement[], index: number): TreeKeyAction {
  const row = rows[index];
  const isDir = row.dataset.dir === 'true';
  const isOpen = row.dataset.open === 'true';
  const last = rows.length - 1;
  switch (key) {
    case 'ArrowDown':
      return index < last ? { focus: index + 1 } : null;
    case 'ArrowUp':
      return index > 0 ? { focus: index - 1 } : null;
    case 'Home':
      return { focus: 0 };
    case 'End':
      return { focus: last };
    case 'ArrowRight':
      if (!isDir) return null;
      if (!isOpen) return { open: true };
      return index < last && rowDepth(rows[index + 1]) > rowDepth(row) ? { focus: index + 1 } : null;
    case 'ArrowLeft': {
      if (isDir && isOpen) return { open: false };
      const parent = nearestAncestorRow(rows, index);
      return parent === null ? null : { focus: parent };
    }
    default:
      return null;
  }
}

function focusRow(row: HTMLElement): void {
  row.querySelector<HTMLElement>('.tree__link')?.focus();
}

function sortNodes(nodes: TreeNode[]): TreeNode[] {
  return [...nodes].sort((a, b) => {
    if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
    return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });
  });
}

interface TreeViewProps {
  /** Path of the document or folder currently open, so it can be highlighted. */
  activePath: string;
  onNavigate?: () => void;
}

export function TreeView({ activePath, onNavigate }: TreeViewProps) {
  const { rootId, tree, treeLoading, treeError, reloadTree, canEdit } = useVault();
  const [expanded, setExpanded] = useState<Set<string>>(() => readExpanded(rootId));

  // Reveal whatever is open, without collapsing what the user opened by hand.
  const [revealedPath, setRevealedPath] = useState(activePath);
  if (revealedPath !== activePath) {
    setRevealedPath(activePath);
    const ancestors = ancestorsOf(activePath);
    if (ancestors.length > 0) {
      setExpanded((current) => {
        if (ancestors.every((ancestor) => current.has(ancestor))) return current;
        const next = new Set(current);
        for (const ancestor of ancestors) next.add(ancestor);
        return next;
      });
    }
  }

  useEffect(() => {
    try {
      localStorage.setItem(storageKey(rootId), JSON.stringify([...expanded]));
    } catch {
      // Ignore: private browsing and full quotas both land here.
    }
  }, [rootId, expanded]);

  const toggle = useCallback((path: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (!next.delete(path)) next.add(path);
      return next;
    });
  }, []);

  const setOpen = useCallback((path: string, open: boolean) => {
    setExpanded((current) => {
      if (current.has(path) === open) return current;
      const next = new Set(current);
      if (open) next.add(path);
      else next.delete(path);
      return next;
    });
  }, []);

  // One listener on the outer list serves every row, however deep. A key pressed inside
  // the context menu bubbles here through the portal, but its target sits in no row, so
  // it is left to the menu.
  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLUListElement>) => {
      if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return;
      const row = (event.target as HTMLElement).closest<HTMLElement>('[data-tree-row]');
      if (!row) return;
      const rows = treeRows(event.currentTarget);
      const action = resolveTreeKey(event.key, rows, rows.indexOf(row));
      if (!action) return;
      event.preventDefault();
      if ('focus' in action) focusRow(rows[action.focus]);
      else setOpen(row.dataset.path ?? '', action.open);
    },
    [setOpen],
  );

  const sorted = useMemo(() => (tree ? sortNodes(tree) : null), [tree]);

  if (treeError) return <ErrorState error={treeError} onRetry={reloadTree} />;
  if (!sorted) return treeLoading ? <LoadingState label="Loading documents…" /> : null;
  if (sorted.length === 0) {
    return <EmptyState title="This folder is empty" detail={canEdit ? 'Use “New” above to add a note.' : undefined} />;
  }

  return (
    <ul className="tree" onKeyDown={onKeyDown}>
      {sorted.map((node) => (
        <TreeBranch
          key={node.path}
          node={node}
          depth={0}
          rootId={rootId}
          activePath={activePath}
          expanded={expanded}
          onToggle={toggle}
          onNavigate={onNavigate}
        />
      ))}
    </ul>
  );
}

interface TreeBranchProps {
  node: TreeNode;
  depth: number;
  rootId: string;
  activePath: string;
  expanded: Set<string>;
  onToggle: (path: string) => void;
  onNavigate?: () => void;
}

function TreeBranch({ node, depth, rootId, activePath, expanded, onToggle, onNavigate }: TreeBranchProps) {
  const { canEdit } = useVault();
  const actions = useFileActions();
  const [menuAnchor, setMenuAnchor] = useState<{ x: number; y: number } | null>(null);

  const isOpen = expanded.has(node.path);
  const isActive = activePath === node.path;
  const children = node.isDir && node.children ? sortNodes(node.children) : [];

  const menuItems: MenuItem[] = node.isDir
    ? [
        { id: 'new-note', label: 'New note here', icon: 'file-plus', onSelect: () => actions.newNote(node.path) },
        {
          id: 'new-folder',
          label: 'New folder here',
          icon: 'folder-plus',
          onSelect: () => actions.newFolder(node.path),
        },
        { id: 'upload', label: 'Upload files here', icon: 'upload', onSelect: () => actions.upload(node.path) },
        { id: 'rename', label: 'Rename…', icon: 'rename', onSelect: () => actions.rename(node.path, true) },
        { id: 'move', label: 'Move…', icon: 'folder-move', onSelect: () => actions.move(node.path, true) },
        {
          id: 'delete',
          label: 'Delete…',
          icon: 'trash',
          danger: true,
          onSelect: () => actions.remove(node.path, true),
        },
      ]
    : [
        { id: 'rename', label: 'Rename…', icon: 'rename', onSelect: () => actions.rename(node.path, false) },
        { id: 'move', label: 'Move…', icon: 'folder-move', onSelect: () => actions.move(node.path, false) },
        {
          id: 'delete',
          label: 'Delete…',
          icon: 'trash',
          danger: true,
          onSelect: () => actions.remove(node.path, false),
        },
      ];

  const openMenu = (event: React.MouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    setMenuAnchor({ x: rect.left, y: rect.bottom + MENU_OFFSET_PX });
  };

  return (
    <li>
      <div
        className={`tree__row${isActive ? ' tree__row--selected' : ''}`}
        style={{ paddingLeft: depth * INDENT_PER_DEPTH_PX }}
        data-tree-row
        data-path={node.path}
        data-depth={depth}
        data-dir={node.isDir}
        data-open={node.isDir ? isOpen : undefined}
      >
        <button
          type="button"
          className={`tree__twisty${node.isDir ? '' : ' tree__twisty--leaf'}`}
          onClick={() => node.isDir && onToggle(node.path)}
          aria-expanded={node.isDir ? isOpen : undefined}
          aria-label={node.isDir ? `${isOpen ? 'Collapse' : 'Expand'} ${node.name}` : undefined}
          tabIndex={node.isDir ? 0 : -1}
          aria-hidden={node.isDir ? undefined : true}
        >
          <Icon name={isOpen ? 'chevron-down' : 'chevron-right'} />
        </button>

        <Link
          className="tree__link"
          to={node.isDir ? folderRoute(rootId, node.path) : docRoute(rootId, node.path)}
          aria-current={isActive ? 'page' : undefined}
          onClick={onNavigate}
        >
          <span className="kind-icon" aria-hidden="true">
            <Icon name={kindIconName(node.isDir ? 'folder' : node.kind)} />
          </span>
          <span className="tree__label">{node.name}</span>
          <span className="sr-only">{kindLabel(node.isDir ? 'folder' : node.kind)}</span>
        </Link>

        {canEdit ? (
          <button
            type="button"
            className="tree__more"
            onClick={openMenu}
            aria-haspopup="menu"
            aria-expanded={menuAnchor !== null}
            aria-label={`Actions for ${node.name}`}
          >
            <Icon name="more" />
          </button>
        ) : null}
      </div>

      {menuAnchor ? (
        <ContextMenu
          items={menuItems}
          anchor={menuAnchor}
          label={`Actions for ${node.name}`}
          onClose={() => setMenuAnchor(null)}
        />
      ) : null}

      {node.isDir && isOpen && children.length > 0 ? (
        <ul className="tree">
          {children.map((child) => (
            <TreeBranch
              key={child.path}
              node={child}
              depth={depth + 1}
              rootId={rootId}
              activePath={activePath}
              expanded={expanded}
              onToggle={onToggle}
              onNavigate={onNavigate}
            />
          ))}
        </ul>
      ) : null}
    </li>
  );
}
