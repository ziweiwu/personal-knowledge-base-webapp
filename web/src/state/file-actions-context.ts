import { createContext, useContext } from 'react';

export interface FileActions {
  newNote: (directoryPath: string) => void;
  newFolder: (directoryPath: string) => void;
  rename: (path: string, isDir: boolean) => void;
  remove: (path: string, isDir: boolean) => void;
  upload: (directoryPath: string) => void;
}

export const FileActionsContext = createContext<FileActions | null>(null);

export function useFileActions(): FileActions {
  const value = useContext(FileActionsContext);
  if (!value) throw new Error('useFileActions must be used inside <FileActionsProvider>');
  return value;
}
