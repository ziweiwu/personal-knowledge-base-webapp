import { createContext, useContext } from 'react';
import type { SessionInfo } from '../api/types';

export type AuthStatus = 'checking' | 'authenticated' | 'anonymous';

export interface AuthContextValue {
  status: AuthStatus;
  session: SessionInfo | null;
  signIn: (email: string, password: string) => Promise<void>;
  signOut: () => Promise<void>;
}

export const AuthContext = createContext<AuthContextValue | null>(null);

export function useAuth(): AuthContextValue {
  const value = useContext(AuthContext);
  if (!value) throw new Error('useAuth must be used inside <AuthProvider>');
  return value;
}
