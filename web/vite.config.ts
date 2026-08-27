import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

const API_TARGET = 'http://127.0.0.1:4321';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: API_TARGET,
        changeOrigin: false,
        // SSE must not be buffered by the dev proxy.
        ws: false,
      },
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
    // Mermaid and CodeMirror are reached only through dynamic imports, so the
    // default chunking already keeps them out of the entry bundle — and mermaid
    // splits its own per-diagram modules, which manual chunking would undo.
    chunkSizeWarningLimit: 900,
  },
});
