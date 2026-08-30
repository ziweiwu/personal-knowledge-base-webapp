import { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useParams } from 'react-router-dom';
import { baseName, parentPath } from '../../api/paths';
import { useIsDesktop } from '../../hooks/useMediaQuery';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { useBodyScrollLock } from '../../hooks/useBodyScrollLock';
import { useScrollRestoration } from '../../hooks/useScrollRestoration';
import { useFocusMode } from '../../hooks/useFocusMode';
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

/** Whether a key event came from somewhere the user is typing. */
function isTypingTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  return element.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(element.tagName);
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
  const [focusNotice, setFocusNotice] = useState('');
  const focus = useFocusMode();
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  const mainRef = useRef<HTMLElement>(null);

  useScrollRestoration(mainRef);

  const closeDrawer = useCallback(() => setDrawerOpen(false), []);

  // Escape closes the topmost layer: the drawer outranks focus mode, and the search
  // palette binds its own so it is never reached through here while open.
  const escapeClosesDrawer = drawerOpen && !isDesktop;
  const escapeLeavesFocus = focus.focused && !searchOpen;
  useEscapeKey(escapeClosesDrawer ? closeDrawer : escapeLeavesFocus ? focus.exit : null);
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

  // A bare letter, so it is guarded against every context where it would be a keystroke
  // rather than a command.
  const canFocus = mode === 'doc';
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== 'f') return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (!canFocus || searchOpen || isTypingTarget(event.target)) return;
      event.preventDefault();
      focus.toggle();
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [canFocus, searchOpen, focus]);

  // A folder or tag listing is navigation; there is no document to focus on, and the
  // chrome focus mode hides is the whole page.
  useEffect(() => {
    if (!canFocus && focus.focused) focus.exit();
  }, [canFocus, focus]);

  // Chrome appearing and disappearing is a silent event to a screen reader.
  const hasFocusedOnce = useRef(false);
  useEffect(() => {
    if (!focus.focused && !hasFocusedOnce.current) return;
    hasFocusedOnce.current = true;
    setFocusNotice(focus.focused ? 'Focus reading on. Press Escape to exit.' : 'Focus reading off.');
  }, [focus.focused]);

  const rootName = root?.name ?? roots.find((candidate) => candidate.id === rootId)?.name ?? rootId;
  const currentDirectory = mode === 'doc' ? parentPath(path) : mode === 'tag' ? '' : path;
  const title = pageTitle || (mode === 'doc' ? baseName(path) : baseName(path) || rootName);

  useEffect(() => {
    document.title = title ? `${title} · kbviewer` : 'kbviewer';
  }, [title]);

  const drawerHidden = !isDesktop && !drawerOpen;
  // While the drawer overlays the page, everything behind the scrim must leave
  // the tab order and the accessibility tree — otherwise Tab walks straight
  // through to the breadcrumb and search button the scrim is covering.
  const behindDrawer = !isDesktop && drawerOpen;

  return (
    <div className={`app-shell${focus.focused ? ' app-shell--focus' : ''}`}>
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

        {canFocus ? (
          <button
            type="button"
            className="btn btn--icon"
            onClick={focus.toggle}
            aria-pressed={focus.focused}
            aria-keyshortcuts="F"
            inert={behindDrawer}
          >
            <span aria-hidden="true">⤢</span>
            <span className="sr-only">Focus reading mode</span>
          </button>
        ) : null}

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

      {/* The only control left on screen in focus mode, and on a touch device the only
          way out at all — there is no Escape key on a phone. */}
      {focus.focused ? (
        <button type="button" className="focus-exit" onClick={focus.exit} aria-keyshortcuts="Escape">
          <span aria-hidden="true">✕</span>
          <span className="sr-only">Exit focus reading mode</span>
        </button>
      ) : null}

      <div className="sr-only" role="status" aria-live="polite">
        {focusNotice}
      </div>

      {searchOpen ? <SearchPalette rootId={rootId} rootName={rootName} onClose={() => setSearchOpen(false)} /> : null}
    </div>
  );
}
