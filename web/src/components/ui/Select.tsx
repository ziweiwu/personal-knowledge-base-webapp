import type { SelectHTMLAttributes } from 'react';

interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  /** Take the remaining width of a flex row, as the sidebar's collection picker does. */
  grow?: boolean;
}

export function Select({ grow, className, ...rest }: SelectProps) {
  const classes = ['select', grow ? 'select--grow' : '', className ?? ''].filter(Boolean).join(' ');
  return <select className={classes} {...rest} />;
}
