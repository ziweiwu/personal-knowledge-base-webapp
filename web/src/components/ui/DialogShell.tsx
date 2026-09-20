import { useId, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useBodyScrollLock } from '../../hooks/useBodyScrollLock';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useInertBackground } from '../../hooks/useInertBackground';
import { Button } from './Button';
import { Icon } from './Icon';

interface DialogShellProps {
  title: string;
  /** What the close button is called: "Close dialog", "Close contents". */
  closeLabel: string;
  onClose: () => void;
  /** Runs on a click on the scrim; null where a stray click must not dismiss. */
  onBackdrop: (() => void) | null;
  /** The CSS block the scrim, panel, head and title are named after. */
  block: 'modal' | 'toc-sheet';
  /** Extra classes on the panel, such as `modal--wide`. */
  panelClassName?: string;
  /** Rendered inside the panel above the head: a sheet's drag handle. */
  handle?: ReactNode;
  children: ReactNode;
}

interface DialogHeadProps {
  block: 'modal' | 'toc-sheet';
  title: string;
  titleId: string;
  closeLabel: string;
  onClose: () => void;
}

function DialogHead({ block, title, titleId, closeLabel, onClose }: DialogHeadProps) {
  return (
    <div className={`${block}__head`}>
      <h2 className={`${block}__title`} id={titleId}>
        {title}
      </h2>
      <Button variant="icon" onClick={onClose} aria-label={closeLabel}>
        <Icon name="close" size="md" />
      </Button>
    </div>
  );
}

/**
 * The one dialog chassis: a portal into body, a scrim, a labelled `role="dialog"` panel
 * with a head and a close button, and the four behaviours every modal needs. Modal and
 * the phone's contents sheet only dress it, so a fix here reaches both.
 */
export function DialogShell(props: DialogShellProps) {
  const { title, closeLabel, onClose, onBackdrop, block, panelClassName, handle, children } = props;
  const panelRef = useRef<HTMLDivElement>(null);
  const titleId = useId();

  // Declared before the focus trap so its cleanup runs first: the background is
  // focusable again by the time the trap hands focus back to the opener.
  useInertBackground();
  useFocusTrap(panelRef);
  useEscapeKey(onClose);
  useBodyScrollLock('locked');

  return createPortal(
    <div
      className={`dialog-scrim ${block}-scrim`}
      onMouseDown={(event) => {
        if (onBackdrop && event.target === event.currentTarget) onBackdrop();
      }}
    >
      <div
        className={[block, panelClassName].filter(Boolean).join(' ')}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        ref={panelRef}
      >
        {handle}
        <DialogHead block={block} title={title} titleId={titleId} closeLabel={closeLabel} onClose={onClose} />
        {children}
      </div>
    </div>,
    document.body,
  );
}
