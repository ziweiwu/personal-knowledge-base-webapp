import { useCallback, useMemo, useRef, useState, type ReactNode } from 'react';
import { Banner } from '../components/ui/States';
import { ToastContext, type ToastTone } from './toast-context';

interface Toast {
  id: number;
  message: string;
  tone: ToastTone;
}

/** Long enough to read two sentences; a danger toast waits for a dismissal. */
const AUTO_DISMISS_MS = 7000;

/**
 * One stack of transient messages for the whole app, so a rename, an upload
 * summary and a failed checkbox all land in the same place and are announced
 * the same way, instead of each screen floating its own banner.
 */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(0);

  const dismiss = useCallback((id: number) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const show = useCallback(
    (message: string, tone: ToastTone = 'info') => {
      const id = nextId.current++;
      setToasts((current) => [...current, { id, message, tone }]);
      if (tone !== 'danger') window.setTimeout(() => dismiss(id), AUTO_DISMISS_MS);
    },
    [dismiss],
  );

  const value = useMemo(() => ({ show }), [show]);

  return (
    <ToastContext value={value}>
      {children}
      <div className="toast-stack" aria-live="polite">
        {toasts.map((toast) => (
          <div className="toast" key={toast.id}>
            <Banner tone={toast.tone} onDismiss={() => dismiss(toast.id)}>
              {toast.message}
            </Banner>
          </div>
        ))}
      </div>
    </ToastContext>
  );
}
