import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { docRoute, folderRoute } from '../../api/paths';
import type { TreeNode } from '../../api/types';
import { kindIcon, kindLabel } from '../../lib/format';
import { useFileActions } from '../../state/file-actions-context';
import { useVault } from '../../state/vault-context';
import { ContextMenu, type MenuItem } from '../ui/ContextMenu';
import { EmptyState, ErrorState, LoadingState } from '../ui/States';

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

  const sorted = useMemo(() => (tree ? sortNodes(tree) : null), [tree]);

  if (treeError) return <ErrorState error={treeError} onRetry={reloadTree} />;
  if (!sorted) return treeLoading ? <LoadingState label="Loading documents…" /> : null;
  if (sorted.length === 0) {
    return <EmptyState title="This folder is empty" detail={canEdit ? 'Use “New” above to add a note.' : undefined} />;
  }

  return (
    <ul className="tree">
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
        { id: 'new-note', label: 'New note here', icon: '📝', onSelect: () => actions.newNote(node.path) },
        { id: 'new-folder', label: 'New folder here', icon: '📁', onSelect: () => actions.newFolder(node.path) },
        { id: 'upload', label: 'Upload files here', icon: '⬆️', onSelect: () => actions.upload(node.path) },
        { id: 'rename', label: 'Rename…', icon: '✏️', onSelect: () => actions.rename(node.path, true) },
        { id: 'delete', label: 'Delete…', icon: '🗑️', danger: true, onSelect: () => actions.remove(node.path, true) },
      ]
    : [
        { id: 'rename', label: 'Rename…', icon: '✏️', onSelect: () => actions.rename(node.path, false) },
        { id: 'delete', label: 'Delete…', icon: '🗑️', danger: true, onSelect: () => actions.remove(node.path, false) },
      ];

  const openMenu = (event: React.MouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    setMenuAnchor({ x: rect.left, y: rect.bottom + MENU_OFFSET_PX });
  };

  return (
    <li>
      <div className={`tree__row${isActive ? ' tree__row--selected' : ''}`} style={{ paddingLeft: depth * INDENT_PER_DEPTH_PX }}>
        <button
          type="button"
          className={`tree__twisty${node.isDir ? '' : ' tree__twisty--leaf'}`}
          onClick={() => node.isDir && onToggle(node.path)}
          aria-expanded={node.isDir ? isOpen : undefined}
          aria-label={node.isDir ? `${isOpen ? 'Collapse' : 'Expand'} ${node.name}` : undefined}
          tabIndex={node.isDir ? 0 : -1}
          aria-hidden={node.isDir ? undefined : true}
        >
          <span aria-hidden="true">{isOpen ? '▼' : '▶'}</span>
        </button>

        <Link
          className="tree__link"
          to={node.isDir ? folderRoute(rootId, node.path) : docRoute(rootId, node.path)}
          aria-current={isActive ? 'page' : undefined}
          onClick={onNavigate}
        >
          <span className="kind-icon" aria-hidden="true">
            {kindIcon(node.isDir ? 'folder' : node.kind)}
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
            <span aria-hidden="true">⋯</span>
          </button>
        ) : null}
      </div>

      {menuAnchor ? (
        <ContextMenu items={menuItems} anchor={menuAnchor} label={`Actions for ${node.name}`} onClose={() => setMenuAnchor(null)} />
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
