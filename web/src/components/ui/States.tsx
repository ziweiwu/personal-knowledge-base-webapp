import type { ReactNode } from 'react';
import { describeError, type ErrorKind } from '../../lib/errors';
import { Button } from './Button';
import { Icon } from './Icon';

export type StateTone = 'neutral' | 'info' | 'warning' | 'danger';

/** Tray, bulb, triangle, circle: nothing here yet, a hint, attention needed, a failure. */
const TONE_GLYPH: Record<StateTone, ReactNode> = {
  neutral: <Icon name="inbox" size="lg" />,
  info: <Icon name="bulb" size="lg" />,
  warning: <Icon name="warning" size="lg" />,
  danger: <Icon name="alert-circle" size="lg" />,
};

interface StateFrameProps {
  tone: StateTone;
  glyph?: ReactNode;
  title: string;
  detail?: ReactNode;
  role?: 'alert' | 'status';
  children?: ReactNode;
}

/** The one layout every empty, error and not-found screen shares; tone and glyph tell them apart. */
function StateFrame({ tone, glyph, title, detail, role, children }: StateFrameProps) {
  return (
    <div className={`state state--${tone}`} role={role} data-tone={tone}>
      <span className="state__glyph">{glyph ?? TONE_GLYPH[tone]}</span>
      <p className="state__title">{title}</p>
      {detail ? <p className="state__detail">{detail}</p> : null}
      {children}
    </div>
  );
}

export function Spinner({ label = 'Loading' }: { label?: string }) {
  return (
    <span className="spinner" role="status" aria-label={label}>
      <span className="sr-only">{label}</span>
    </span>
  );
}

export function LoadingState({ label = 'Loading…' }: { label?: string }) {
  return (
    <div className="state state--loading">
      <Spinner label={label} />
      <p className="state__detail">{label}</p>
    </div>
  );
}

interface EmptyStateProps {
  title: string;
  detail?: ReactNode;
  tone?: StateTone;
  glyph?: ReactNode;
  children?: ReactNode;
}

export function EmptyState({ title, detail, tone = 'neutral', glyph, children }: EmptyStateProps) {
  return (
    <StateFrame tone={tone} glyph={glyph} title={title} detail={detail}>
      {children}
    </StateFrame>
  );
}

/** Where the address pointed does not exist. Neutral: the app is fine, the path is not. */
export function NotFoundState({
  title,
  detail,
  children,
}: {
  title: string;
  detail?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <StateFrame tone="neutral" glyph={<Icon name="compass" size="lg" />} title={title} detail={detail} role="status">
      {children}
    </StateFrame>
  );
}

/** A failure that only needs patience or a retry reads as a warning; the rest as danger. */
function toneForFailure(kind: ErrorKind): StateTone {
  return kind === 'network' || kind === 'rate-limited' ? 'warning' : 'danger';
}

interface ErrorStateProps {
  error: Error;
  onRetry?: () => void;
  tone?: StateTone;
  glyph?: ReactNode;
}

export function ErrorState({ error, onRetry, tone, glyph }: ErrorStateProps) {
  const { kind, title, detail } = describeError(error);
  const retry = onRetry ? <Button onClick={onRetry}>Try again</Button> : null;
  if (kind === 'not-found' && !tone) {
    return (
      <NotFoundState title={title} detail={detail}>
        {retry}
      </NotFoundState>
    );
  }
  return (
    <StateFrame tone={tone ?? toneForFailure(kind)} glyph={glyph} title={title} detail={detail} role="alert">
      {retry}
    </StateFrame>
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
          <Icon name="close" />
        </Button>
      ) : null}
    </div>
  );
}
