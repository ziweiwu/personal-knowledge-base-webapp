import type { ComponentProps } from 'react';

export type ButtonVariant = 'default' | 'primary' | 'danger' | 'danger-quiet' | 'ghost' | 'icon';

/* ComponentProps rather than ButtonHTMLAttributes so `ref` passes through (React 19). */
interface ButtonProps extends ComponentProps<'button'> {
  variant?: ButtonVariant;
  /** Full width of its row; the login form's submit. */
  block?: boolean;
}

/**
 * The one button. Every variant is a modifier on `.btn` in app.css; this
 * component exists so a call site says `variant="danger"` instead of
 * assembling the class string by hand, and so `type="button"` is the default
 * rather than the thing everyone forgets inside a form.
 */
export function Button({ variant = 'default', block, className, type = 'button', ...rest }: ButtonProps) {
  const classes = ['btn', variant !== 'default' ? `btn--${variant}` : '', block ? 'btn--block' : '', className ?? '']
    .filter(Boolean)
    .join(' ');
  return <button type={type} className={classes} {...rest} />;
}
