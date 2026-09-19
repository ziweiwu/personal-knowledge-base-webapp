import { createContext, useContext } from 'react';
import type { BannerTone } from '../components/ui/States';

/** A toast is a Banner that arrives on its own, so it takes the same tones. */
export type ToastTone = BannerTone;

export interface ToastContextValue {
  /** Show a short, self-dismissing message at the bottom of the screen. */
  show: (message: string, tone?: ToastTone) => void;
}

export const ToastContext = createContext<ToastContextValue | null>(null);

export function useToast(): ToastContextValue {
  const value = useContext(ToastContext);
  if (!value) throw new Error('useToast must be used inside <ToastProvider>');
  return value;
}
