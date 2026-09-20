import { useId, useMemo, type KeyboardEvent } from 'react';
import type { TreeNode } from '../../api/types';

export interface FolderOption {
  path: string;
  name: string;
  depth: number;
  /** A folder that cannot receive the item: the item itself or one of its descendants. */
  disabled: boolean;
}

const ROOT_LABEL = '/ (top level)';

/** True when `candidate` is `folder` itself or sits somewhere beneath it. */
function isWithin(candidate: string, folder: string): boolean {
  return candidate === folder || candidate.startsWith(`${folder}/`);
}

function collectFolders(nodes: TreeNode[], depth: number, blocked: string | null, into: FolderOption[]): void {
  const folders = nodes.filter((node) => node.isDir).sort((left, right) => left.name.localeCompare(right.name));
  for (const folder of folders) {
    into.push({
      path: folder.path,
      name: folder.name,
      depth,
      disabled: blocked !== null && isWithin(folder.path, blocked),
    });
    if (folder.children) collectFolders(folder.children, depth + 1, blocked, into);
  }
}

/** Every folder in the tree, depth first, with the top level as the first entry. */
function flattenFolders(tree: TreeNode[], blocked: string | null): FolderOption[] {
  const options: FolderOption[] = [{ path: '', name: ROOT_LABEL, depth: 0, disabled: false }];
  collectFolders(tree, 1, blocked, options);
  return options;
}

interface FolderPickerProps {
  tree: TreeNode[];
  value: string;
  onChange: (path: string) => void;
  /** The folder being moved, if any, so it and its subtree are offered but greyed out. */
  blocked?: string | null;
  disabled?: boolean;
  label: string;
}

/**
 * A single-select list of the root's folders, driven like a radio group: arrow
 * keys move the choice, so a phone with an external keyboard and a desktop
 * both work without a pointer.
 */
export function FolderPicker({ tree, value, onChange, blocked = null, disabled, label }: FolderPickerProps) {
  const options = useMemo(() => flattenFolders(tree, blocked), [tree, blocked]);
  const groupId = useId();
  const enabled = options.filter((option) => !option.disabled);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const direction = event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0;
    if (direction === 0 || enabled.length === 0) return;
    event.preventDefault();
    const current = enabled.findIndex((option) => option.path === value);
    const next = enabled[(current + direction + enabled.length) % enabled.length];
    onChange(next.path);
    document.getElementById(`${groupId}-${next.path}`)?.focus();
  };

  return (
    <div className="folder-picker" role="radiogroup" aria-label={label} onKeyDown={onKeyDown}>
      {options.map((option) => {
        const checked = option.path === value;
        return (
          <button
            key={option.path}
            id={`${groupId}-${option.path}`}
            type="button"
            role="radio"
            aria-checked={checked}
            className={`folder-picker__item${checked ? ' folder-picker__item--checked' : ''}`}
            style={{ paddingLeft: `calc(var(--space-3) + ${option.depth} * var(--space-5))` }}
            disabled={disabled || option.disabled}
            tabIndex={checked ? 0 : -1}
            onClick={() => onChange(option.path)}
          >
            <span className="kind-icon" aria-hidden="true">
              📁
            </span>
            <span className="folder-picker__name">{option.name}</span>
          </button>
        );
      })}
    </div>
  );
}
