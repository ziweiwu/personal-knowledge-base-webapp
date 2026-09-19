import { Modal } from './Modal';
import { Button } from './Button';

interface ConfirmDialogProps {
  title: string;
  message: string;
  detail?: string;
  confirmLabel: string;
  danger?: boolean;
  busy?: boolean;
  error?: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  title,
  message,
  detail,
  confirmLabel,
  danger,
  busy,
  error,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  return (
    <Modal
      title={title}
      onClose={onCancel}
      dismissOnBackdrop={false}
      footer={
        <>
          {/* A destructive dialog opens with Cancel focused. Enter carried over from a
              previous dialog then dismisses this one instead of deleting a file; the
              user has to move to the confirm button deliberately. */}
          <Button onClick={onCancel} disabled={busy} data-autofocus={danger || undefined}>
            Cancel
          </Button>
          <Button
            variant={danger ? 'danger' : 'primary'}
            onClick={onConfirm}
            disabled={busy}
            data-autofocus={danger ? undefined : true}
          >
            {busy ? 'Working…' : confirmLabel}
          </Button>
        </>
      }
    >
      <p>{message}</p>
      {detail ? <p className="state__detail state__detail--start">{detail}</p> : null}
      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}
    </Modal>
  );
}
