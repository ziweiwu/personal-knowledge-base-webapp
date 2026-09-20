import { useMemo } from 'react';
import type { SaveConflict } from '../../api/types';
import { formatDateTime } from '../../lib/format';
import { type DiffLine, type LineDiff, diffLines } from '../../lib/lineDiff';
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

interface PaneProps {
  id: string;
  title: string;
  subtitle: string;
  lines: DiffLine[];
}

/** The first changed line carries the pane's id so the summary can jump to it. */
function firstChangedIndex(lines: DiffLine[]): number {
  return lines.findIndex((line) => line.kind === 'changed');
}

function Pane({ id, title, subtitle, lines }: PaneProps) {
  const firstChanged = firstChangedIndex(lines);
  return (
    <div className="conflict__pane">
      <div className="conflict__pane-head">
        {title}
        <span className="conflict__pane-sub">{subtitle}</span>
      </div>
      <pre className="conflict__pre" tabIndex={0}>
        {lines.map((line, index) => (
          <span
            key={index}
            id={index === firstChanged ? id : undefined}
            tabIndex={index === firstChanged ? -1 : undefined}
            className={line.kind === 'changed' ? 'conflict__line conflict__line--changed' : 'conflict__line'}
          >
            {line.kind === 'changed' ? <span className="sr-only">changed line: </span> : null}
            {line.text}
            {'\n'}
          </span>
        ))}
      </pre>
    </div>
  );
}

function describeDiff(diff: LineDiff): string {
  if (diff.tooLarge) return 'Too large to compare line by line.';
  if (diff.changed === 0) return 'Identical apart from whitespace or line endings.';
  return diff.changed === 1 ? '1 line differs.' : `${diff.changed} lines differ.`;
}

function jumpTo(id: string): void {
  const line = document.getElementById(id);
  if (!line) return;
  line.scrollIntoView({ block: 'center' });
  line.focus();
}

function DiffSummary({ diff }: { diff: LineDiff }) {
  return (
    <p className="conflict__summary" role="status">
      <span>{describeDiff(diff)}</span>
      {diff.changed > 0 ? (
        <Button variant="ghost" onClick={() => jumpTo('conflict-first-change-mine')}>
          Jump to first change
        </Button>
      ) : null}
    </p>
  );
}

/**
 * Shown on HTTP 409. Both versions are on screen and neither is applied until
 * the user picks one — Obsidian may well have the same note open.
 */
export function ConflictDialog({ conflict, busy, error, onKeepMine, onTakeTheirs, onCancel }: ConflictDialogProps) {
  const diff = useMemo(() => diffLines(conflict.yourContent, conflict.diskContent), [conflict]);
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
      <DiffSummary diff={diff} />
      <div className="conflict">
        <Pane
          id="conflict-first-change-mine"
          title="Your version"
          subtitle="the buffer in this editor"
          lines={diff.left}
        />
        <Pane
          id="conflict-first-change-disk"
          title="On disk"
          subtitle={`modified ${formatDateTime(conflict.diskMtimeMs)}`}
          lines={diff.right}
        />
      </div>
    </Modal>
  );
}
