import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { fetchSession, login, logout, onUnauthorized } from '../api/client';
import type { SessionInfo } from '../api/types';
import { AuthContext, type AuthStatus } from './auth-context';

export function AuthProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<AuthStatus>('checking');
  const [session, setSession] = useState<SessionInfo | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    fetchSession(controller.signal)
      .then((restored) => {
        setSession(restored);
        setStatus('authenticated');
      })
      .catch(() => {
        if (controller.signal.aborted) return;
        setSession(null);
        setStatus('anonymous');
      });
    return () => controller.abort();
  }, []);

  // A 401 from any request means the cookie is gone; drop straight to signed out
  // so the router can send the user to /login instead of showing a broken pane.
  useEffect(
    () =>
      onUnauthorized(() => {
        setSession(null);
        setStatus('anonymous');
      }),
    [],
  );

  const signIn = useCallback(async (email: string, password: string) => {
    await login(email, password);
    const signedIn = await fetchSession().catch(() => ({ email }) as SessionInfo);
    setSession(signedIn);
    setStatus('authenticated');
  }, []);

  const signOut = useCallback(async () => {
    try {
      await logout();
    } finally {
      setSession(null);
      setStatus('anonymous');
    }
  }, []);

  const value = useMemo(() => ({ status, session, signIn, signOut }), [status, session, signIn, signOut]);

  return <AuthContext value={value}>{children}</AuthContext>;
}
