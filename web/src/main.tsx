import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { App } from './App';
import { ErrorBoundary } from './components/ErrorBoundary';
import { AuthProvider } from './state/AuthProvider';
import { ThemeProvider } from './state/ThemeProvider';
import './styles/tokens.css';
import './styles/base.css';
import './styles/app.css';
import './styles/content.css';

// Development fixtures, imported dynamically so the module cannot reach a build.
if (import.meta.env.VITE_MOCK === '1') {
  const { installMockTransport } = await import('./api/mock');
  installMockTransport();
}

const container = document.getElementById('root');
if (!container) throw new Error('Missing #root element');

createRoot(container).render(
  <StrictMode>
    <ErrorBoundary>
      <ThemeProvider>
        <AuthProvider>
          <BrowserRouter>
            <App />
          </BrowserRouter>
        </AuthProvider>
      </ThemeProvider>
    </ErrorBoundary>
  </StrictMode>,
);
