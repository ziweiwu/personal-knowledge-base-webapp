/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "1" turns on the in-browser fixture backend; see src/api/mock.ts. */
  readonly VITE_MOCK?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
