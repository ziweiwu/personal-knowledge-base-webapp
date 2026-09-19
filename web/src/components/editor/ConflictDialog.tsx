import type { SaveConflict } from '../../api/types';
import { formatDateTime } from '../../lib/format';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { FormError } from '../ui/States';

interface ConflictDialogProps {
  conflict: SaveConflict;
  busy: boolean;
  error: string | null;
  /** Overwrite the file on disk with the buffer, using the disk mtime as the new base. */
  onKeepMine: () => void;
  /** Discard the buffer and adopt what is on disk. */
  onTakeTheirs: () => void;
  onCancel: () => void;
}

function Pane({ title, subtitle, content }: { title: string; subtitle: string; content: string }) {
  return (
    <div className="conflict__pane">
      <div className="conflict__pane-head">
        {title}
        <span className="conflict__pane-sub">{subtitle}</span>
      </div>
      <pre className="conflict__pre" tabIndex={0}>
        {content}
      </pre>
    </div>
  );
}

/**
 * Shown on HTTP 409. Both versions are on screen and neither is applied until
 * the user picks one — Obsidian may well have the same note open.
 */
export function ConflictDialog({ conflict, busy, error, onKeepMine, onTakeTheirs, onCancel }: ConflictDialogProps) {
  return (
    <Modal
      title="This file changed on disk"
      onClose={onCancel}
      wide
      dismissOnBackdrop={false}
      footer={
        <>
          <Button onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          <Button onClick={onTakeTheirs} disabled={busy}>
            Take theirs (discard my edits)
          </Button>
          <Button variant="primary" onClick={onKeepMine} disabled={busy} data-autofocus>
            {busy ? 'Saving…' : 'Keep mine (overwrite disk)'}
          </Button>
        </>
      }
    >
      <p>
        <strong>{conflict.path}</strong> was modified by something else — most likely Obsidian — after you opened it.
        Nothing has been written yet.
      </p>
      <FormError message={error ?? null} />
      <div className="conflict">
        <Pane title="Your version" subtitle="the buffer in this editor" content={conflict.yourContent} />
        <Pane
          title="On disk"
          subtitle={`modified ${formatDateTime(conflict.diskMtimeMs)}`}
          content={conflict.diskContent}
        />
      </div>
    </Modal>
  );
}
