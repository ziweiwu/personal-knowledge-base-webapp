import type { ReactNode } from 'react';
import { describeError } from '../../lib/errors';
import { Button } from './Button';

export function Spinner({ label = 'Loading' }: { label?: string }) {
  return (
    <span className="spinner" role="status" aria-label={label}>
      <span className="sr-only">{label}</span>
    </span>
  );
}

export function LoadingState({ label = 'Loading…' }: { label?: string }) {
  return (
    <div className="state">
      <Spinner label={label} />
      <p className="state__detail">{label}</p>
    </div>
  );
}

export function EmptyState({ title, detail, children }: { title: string; detail?: string; children?: ReactNode }) {
  return (
    <div className="state">
      <p className="state__title">{title}</p>
      {detail ? <p className="state__detail">{detail}</p> : null}
      {children}
    </div>
  );
}

export function ErrorState({ error, onRetry }: { error: Error; onRetry?: () => void }) {
  const { title, detail } = describeError(error);
  return (
    <div className="state" role="alert">
      <p className="state__title">{title}</p>
      <p className="state__detail">{detail}</p>
      {onRetry ? <Button onClick={onRetry}>Try again</Button> : null}
    </div>
  );
}

/** An inline error under a form or dialog body, announced when it appears. */
export function FormError({ id, message }: { id?: string; message: string | null }) {
  if (!message) return null;
  return (
    <p className="form-error" id={id} role="alert">
      {message}
    </p>
  );
}

export type BannerTone = 'info' | 'warning' | 'danger';

interface BannerProps {
  tone?: BannerTone;
  children: ReactNode;
  actions?: ReactNode;
  onDismiss?: () => void;
}

export function Banner({ tone = 'info', children, actions, onDismiss }: BannerProps) {
  return (
    <div className={`banner banner--${tone}`} role={tone === 'danger' ? 'alert' : 'status'}>
      <div className="banner__body">{children}</div>
      {actions ? <div className="banner__actions">{actions}</div> : null}
      {onDismiss ? (
        <Button variant="ghost" onClick={onDismiss} aria-label="Dismiss message">
          ✕
        </Button>
      ) : null}
    </div>
  );
}
