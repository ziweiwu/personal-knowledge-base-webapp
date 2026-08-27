import { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useParams } from 'react-router-dom';
import { baseName, parentPath } from '../../api/paths';
import { useIsDesktop } from '../../hooks/useMediaQuery';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { useBodyScrollLock } from '../../hooks/useBodyScrollLock';
import { useScrollRestoration } from '../../hooks/useScrollRestoration';
import { FileActionsProvider } from '../files/FileActionsProvider';
import { useRoots } from '../../state/roots-context';
import { VaultProvider } from '../../state/VaultProvider';
import { useVault } from '../../state/vault-context';
import { DocumentPage } from '../../pages/DocumentPage';
import { FolderPage } from '../../pages/FolderPage';
import { TagPage } from '../../pages/TagPage';
import { SearchPalette } from '../search/SearchPalette';
import { Breadcrumbs } from './Breadcrumbs';
import { Sidebar } from './Sidebar';
import { ThemeToggle } from './ThemeToggle';

export type VaultMode = 'doc' | 'folder' | 'tag';

/** Splat params arrive percent-encoded; the API layer re-encodes per segment. */
function decodeSplat(splat: string | undefined): string {
  return (splat ?? '')
    .split('/')
    .filter(Boolean)
    .map((segment) => {
      try {
        return decodeURIComponent(segment);
      } catch {
        return segment;
      }
    })
    .join('/');
}

export function VaultLayout({ mode }: { mode: VaultMode }) {
  const params = useParams();
  const rootId = params.rootId ?? '';
  const path = decodeSplat(params['*']);

  return (
    <VaultProvider rootId={rootId}>
      <FileActionsProvider>
        <VaultShell mode={mode} path={path} />
      </FileActionsProvider>
    </VaultProvider>
  );
}

function VaultShell({ mode, path }: { mode: VaultMode; path: string }) {
  const { rootId, root } = useVault();
  const { roots } = useRoots();
  const location = useLocation();
  const isDesktop = useIsDesktop();

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [pageTitle, setPageTitle] = useState<string>('');
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  const mainRef = useRef<HTMLElement>(null);

  useScrollRestoration(mainRef);

  const closeDrawer = useCallback(() => setDrawerOpen(false), []);
  useEscapeKey(drawerOpen && !isDesktop ? closeDrawer : null);
  useBodyScrollLock(drawerOpen && !isDesktop ? 'locked' : 'scrollable');

  // A navigation on mobile should reveal the document, not leave the drawer open.
  const [shownPath, setShownPath] = useState(location.pathname);
  if (shownPath !== location.pathname) {
    setShownPath(location.pathname);
    setDrawerOpen(false);
  }

  // Return focus to the menu button when the drawer *closes*, but never on first
  // render: focusing it on load makes it the active element on every navigation, which
  // pre-empts the skip link (so it can only be reached by shift-tabbing backwards) and
  // drops a screen-reader user on "Open document list" instead of the document heading.
  const drawerWasOpen = useRef(false);
  useEffect(() => {
    if (drawerWasOpen.current && !drawerOpen) {
      menuButtonRef.current?.focus({ preventScroll: true });
    }
    drawerWasOpen.current = drawerOpen;
  }, [drawerOpen]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setSearchOpen(true);
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, []);

  const rootName = root?.name ?? roots.find((candidate) => candidate.id === rootId)?.name ?? rootId;
  const currentDirectory = mode === 'doc' ? parentPath(path) : mode === 'tag' ? '' : path;
  const title = pageTitle || (mode === 'doc' ? baseName(path) : baseName(path) || rootName);

  useEffect(() => {
    document.title = title ? `${title} · kbview` : 'kbview';
  }, [title]);

  const drawerHidden = !isDesktop && !drawerOpen;
  // While the drawer overlays the page, everything behind the scrim must leave
  // the tab order and the accessibility tree — otherwise Tab walks straight
  // through to the breadcrumb and search button the scrim is covering.
  const behindDrawer = !isDesktop && drawerOpen;

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>

      <header className="topbar">
        <button
          type="button"
          className="btn btn--icon only-mobile"
          ref={menuButtonRef}
          onClick={() => setDrawerOpen((open) => !open)}
          aria-label={drawerOpen ? 'Close document list' : 'Open document list'}
          aria-expanded={drawerOpen}
          aria-controls="sidebar-drawer"
        >
          <span aria-hidden="true">☰</span>
        </button>

        {/* The toolbar outranks the scrim so its toggle can close the drawer again;
            everything else in it is page chrome and is neutralised alongside <main>. */}
        <div className="topbar__title" inert={behindDrawer}>
          {title}
          <Breadcrumbs rootId={rootId} rootName={rootName} path={path} mode={mode} />
        </div>

        <button
          type="button"
          className="search-trigger"
          onClick={() => setSearchOpen(true)}
          aria-keyshortcuts="Meta+K Control+K"
          inert={behindDrawer}
        >
          <span aria-hidden="true">🔎</span>
          <span className="search-trigger__label only-desktop">Search</span>
          <kbd className="only-desktop" aria-hidden="true">
            ⌘K
          </kbd>
          <span className="sr-only">Search this collection</span>
        </button>

        <span className="only-mobile" inert={behindDrawer}>
          <ThemeToggle />
        </span>
      </header>

      <div className="app-body">
        {drawerOpen && !isDesktop ? (
          // Decorative: closing is already reachable by the toolbar toggle and Escape.
          // As a labelled <button> it was a second tab stop announcing the same name as
          // the toggle, which is ambiguous to a screen reader for no added capability.
          <div className="scrim" aria-hidden="true" onClick={closeDrawer} />
        ) : null}

        <nav
          id="sidebar-drawer"
          className={`sidebar${drawerOpen ? ' sidebar--open' : ''}`}
          aria-label="Documents"
          aria-hidden={drawerHidden}
          inert={drawerHidden}
        >
          <Sidebar activePath={path} currentDirectory={currentDirectory} onNavigate={closeDrawer} />
        </nav>

        <main className="main-pane" id="main-content" tabIndex={-1} ref={mainRef} inert={behindDrawer}>
          {mode === 'tag' ? (
            <TagPage rootId={rootId} tag={path} onTitleChange={setPageTitle} />
          ) : mode === 'doc' ? (
            <DocumentPage rootId={rootId} path={path} onTitleChange={setPageTitle} />
          ) : (
            <FolderPage rootId={rootId} path={path} onTitleChange={setPageTitle} />
          )}
        </main>
      </div>

      {searchOpen ? <SearchPalette rootId={rootId} rootName={rootName} onClose={() => setSearchOpen(false)} /> : null}
    </div>
  );
}
