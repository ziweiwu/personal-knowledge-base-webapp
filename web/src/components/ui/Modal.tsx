import type { ReactNode } from 'react';
import { DialogShell } from './DialogShell';

interface ModalProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  /** Set for destructive or lossy dialogs, where a stray click must not dismiss. */
  dismissOnBackdrop?: boolean;
}

export function Modal({ title, onClose, children, footer, wide, dismissOnBackdrop = true }: ModalProps) {
  return (
    <DialogShell
      title={title}
      closeLabel="Close dialog"
      onClose={onClose}
      onBackdrop={dismissOnBackdrop ? onClose : null}
      block="modal"
      panelClassName={wide ? 'modal--wide' : undefined}
    >
      <div className="modal__body">{children}</div>
      {footer ? <div className="modal__foot">{footer}</div> : null}
    </DialogShell>
  );
}
