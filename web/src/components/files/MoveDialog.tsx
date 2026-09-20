import { useState } from 'react';
import type { TreeNode } from '../../api/types';
import { baseName, parentPath } from '../../api/paths';
import { Button } from '../ui/Button';
import { Modal } from '../ui/Modal';
import { FormError } from '../ui/States';
import { FolderPicker } from './FolderPicker';

interface MoveDialogProps {
  path: string;
  isDir: boolean;
  tree: TreeNode[];
  linksRewritten: boolean;
  busy: boolean;
  error: string | null;
  /** Called with the chosen folder; the parent joins the item's name onto it. */
  onSubmit: (folder: string) => void;
  onCancel: () => void;
}

function describeFolder(folder: string): string {
  return folder ? `“${folder}”` : 'the top level';
}

export function MoveDialog({ path, isDir, tree, linksRewritten, busy, error, onSubmit, onCancel }: MoveDialogProps) {
  const origin = parentPath(path);
  const [folder, setFolder] = useState(origin);
  const unchanged = folder === origin;
  const name = baseName(path);

  return (
    <Modal
      title={isDir ? 'Move folder' : 'Move note'}
      onClose={onCancel}
      dismissOnBackdrop={false}
      footer={
        <>
          <Button onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => onSubmit(folder)} disabled={busy || unchanged}>
            {busy ? 'Moving…' : `Move to ${describeFolder(folder)}`}
          </Button>
        </>
      }
    >
      <p className="move__lead">
        Choose where <strong>{name}</strong> should live. It is in {describeFolder(origin)} now.
      </p>
      <FolderPicker
        tree={tree}
        value={folder}
        onChange={setFolder}
        blocked={isDir ? path : null}
        disabled={busy}
        label={`Destination folder for ${name}`}
      />
      <p className="state__detail state__detail--start">
        {linksRewritten
          ? 'Links pointing at this file will be rewritten.'
          : 'Links pointing at this file are not rewritten in a plain folder; update them by hand.'}
      </p>
      <FormError message={error} />
    </Modal>
  );
}
