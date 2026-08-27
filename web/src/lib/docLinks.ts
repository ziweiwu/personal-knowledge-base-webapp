import { docRoute, fileUrl, parentPath } from '../api/paths';

const EXTERNAL_SCHEME = /^[a-z][a-z0-9+.-]*:/i;

export function isExternalHref(href: string): boolean {
  return href.startsWith('//') || EXTERNAL_SCHEME.test(href);
}

/**
 * `decodeURIComponent` throws on a malformed escape such as a lone `%`, which a filename
 * can legitimately contain. This runs inside a layout effect with no error boundary above
 * it, so an unguarded throw unmounts the whole app and leaves a blank page. Falling back
 * to the raw text costs at most a link that does not resolve.
 */
function decodeOrPassThrough(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

/** Resolves a document-relative href (`../notes/a.md`) against the open document. */
export function resolveVaultPath(fromDocPath: string, href: string): string {
  const target = decodeOrPassThrough(href.split('#')[0].split('?')[0]);
  if (!target) return fromDocPath;

  const base = target.startsWith('/') ? [] : parentPath(fromDocPath).split('/').filter(Boolean);
  for (const segment of target.split('/')) {
    if (segment === '' || segment === '.') continue;
    if (segment === '..') base.pop();
    else base.push(segment);
  }
  return base.join('/');
}

/** Attachments referenced with a vault-relative path need the raw-bytes route. */
export function resolveAssetUrl(rootId: string, fromDocPath: string, src: string): string | null {
  if (isExternalHref(src) || src.startsWith('data:') || src.startsWith('/api/')) return null;
  return fileUrl(rootId, resolveVaultPath(fromDocPath, src));
}

/** In-app route for a link inside rendered document HTML, or null to leave it alone. */
export function resolveInternalRoute(rootId: string, fromDocPath: string, href: string): string | null {
  // Already an app route (documents, folders, tags): leave it exactly as rendered.
  if (/^\/(n|f|t)\//.test(href)) return href;
  if (!href || href.startsWith('#') || isExternalHref(href)) return null;
  if (href.startsWith('/n/') || href.startsWith('/f/')) return href;
  if (href.startsWith('/api/')) return null;

  const fragment = href.includes('#') ? href.slice(href.indexOf('#')) : '';
  const path = resolveVaultPath(fromDocPath, href);
  if (!path) return null;
  return `${docRoute(rootId, path)}${fragment}`;
}
