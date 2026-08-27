import { Modal } from './Modal';

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
          <button type="button" className="btn" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button
            type="button"
            className={`btn ${danger ? 'btn--danger' : 'btn--primary'}`}
            onClick={onConfirm}
            disabled={busy}
            data-autofocus
          >
            {busy ? 'Working…' : confirmLabel}
          </button>
        </>
      }
    >
      <p>{message}</p>
      {detail ? <p className="state__detail" style={{ textAlign: 'left' }}>{detail}</p> : null}
      {error ? <p className="form-error">{error}</p> : null}
    </Modal>
  );
}
