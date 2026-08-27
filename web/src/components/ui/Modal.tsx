import { useId, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useBodyScrollLock } from '../../hooks/useBodyScrollLock';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { useFocusTrap } from '../../hooks/useFocusTrap';

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
  const panelRef = useRef<HTMLDivElement>(null);
  const titleId = useId();

  useFocusTrap(panelRef);
  useEscapeKey(onClose);
  useBodyScrollLock('locked');

  return createPortal(
    <div
      className="modal-scrim"
      onMouseDown={(event) => {
        if (dismissOnBackdrop && event.target === event.currentTarget) onClose();
      }}
    >
      <div className={`modal${wide ? ' modal--wide' : ''}`} role="dialog" aria-modal="true" aria-labelledby={titleId} ref={panelRef}>
        <div className="modal__head">
          <h2 className="modal__title" id={titleId}>
            {title}
          </h2>
          <button type="button" className="btn btn--icon" onClick={onClose} aria-label="Close dialog">
            ✕
          </button>
        </div>
        <div className="modal__body">{children}</div>
        {footer ? <div className="modal__foot">{footer}</div> : null}
      </div>
    </div>,
    document.body,
  );
}
