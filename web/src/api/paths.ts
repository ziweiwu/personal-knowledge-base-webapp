/**
 * URL construction for the `/api/<verb>/<rootId>/<path...>` route family.
 *
 * The trailing path is a real path, not a single parameter: each segment is
 * encoded on its own so that `/` keeps its meaning while `#`, `?`, spaces and
 * non-ASCII characters in a filename do not break the URL.
 */

export const API_BASE = '/api';

export function encodePath(path: string): string {
  return path
    .split('/')
    .filter((segment) => segment.length > 0)
    .map(encodeURIComponent)
    .join('/');
}

export function resourceUrl(verb: string, rootId: string, path: string): string {
  const encoded = encodePath(path);
  const suffix = encoded ? `/${encoded}` : '';
  return `${API_BASE}/${verb}/${encodeURIComponent(rootId)}${suffix}`;
}

/** Raw bytes: images, PDFs, downloads. Used directly as `src`/`href`. */
export function fileUrl(rootId: string, path: string): string {
  return resourceUrl('file', rootId, path);
}

/** Plain-text source, for the editor. */
export function rawUrl(rootId: string, path: string): string {
  return resourceUrl('raw', rootId, path);
}

export function docUrl(rootId: string, path: string): string {
  return resourceUrl('doc', rootId, path);
}

export function folderUrl(rootId: string, path: string): string {
  return resourceUrl('folder', rootId, path);
}

/** In-app route for a document. */
export function docRoute(rootId: string, path: string): string {
  const encoded = encodePath(path);
  return `/n/${encodeURIComponent(rootId)}${encoded ? `/${encoded}` : ''}`;
}

/** Folder browsing lives under `/f/`, kept apart from the `/n/` document view. */
export function folderRoute(rootId: string, path: string): string {
  const encoded = encodePath(path);
  return `/f/${encodeURIComponent(rootId)}${encoded ? `/${encoded}` : ''}`;
}

export function parentPath(path: string): string {
  const trimmed = path.replace(/\/+$/, '');
  const cut = trimmed.lastIndexOf('/');
  return cut === -1 ? '' : trimmed.slice(0, cut);
}

export function baseName(path: string): string {
  const trimmed = path.replace(/\/+$/, '');
  const cut = trimmed.lastIndexOf('/');
  return cut === -1 ? trimmed : trimmed.slice(cut + 1);
}

export function joinPath(dir: string, name: string): string {
  const cleanDir = dir.replace(/^\/+|\/+$/g, '');
  const cleanName = name.replace(/^\/+|\/+$/g, '');
  return cleanDir ? `${cleanDir}/${cleanName}` : cleanName;
}
