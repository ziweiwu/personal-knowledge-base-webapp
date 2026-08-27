import { Navigate, Outlet, Route, Routes, useLocation } from 'react-router-dom';
import { VaultLayout } from './components/layout/VaultLayout';
import { LoadingState } from './components/ui/States';
import { HomePage } from './pages/HomePage';
import { LoginPage } from './pages/LoginPage';
import { NotFoundPage } from './pages/NotFoundPage';
import { RootsProvider } from './state/RootsProvider';
import { useAuth } from './state/auth-context';

/**
 * Gate for every screen but /login. The requested address is carried in router
 * state so the user lands where they were going, not on a generic home page.
 */
function RequireAuth() {
  const { status } = useAuth();
  const location = useLocation();

  if (status === 'checking') return <LoadingState label="Checking your session…" />;
  if (status === 'anonymous') {
    return <Navigate to="/login" replace state={{ from: `${location.pathname}${location.search}${location.hash}` }} />;
  }
  return (
    <RootsProvider>
      <Outlet />
    </RootsProvider>
  );
}

export function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route element={<RequireAuth />}>
        <Route path="/" element={<HomePage />} />
        <Route path="/n/:rootId/*" element={<VaultLayout mode="doc" />} />
        <Route path="/f/:rootId" element={<VaultLayout mode="folder" />} />
        <Route path="/f/:rootId/*" element={<VaultLayout mode="folder" />} />
        <Route path="/t/:rootId/*" element={<VaultLayout mode="tag" />} />
      </Route>
      <Route path="*" element={<NotFoundPage />} />
    </Routes>
  );
}
