import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState, type RefObject } from 'react';
import { createPortal } from 'react-dom';
import { fetchDocument } from '../../api/client';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { useMediaQuery } from '../../hooks/useMediaQuery';
import { docRouteTarget, resolveInternalRoute } from '../../lib/docLinks';

/** Rest on a link this long before fetching, so sweeping the pointer across prose fetches nothing. */
const OPEN_DELAY_MS = 300;
/** Leaving the link towards the popover must not close it before the pointer arrives. */
const CLOSE_DELAY_MS = 150;
const EXCERPT_MAX_CHARS = 240;
const GAP_PX = 6;
const VIEWPORT_MARGIN_PX = 8;

interface Preview {
  title: string;
  excerpt: string;
}

interface Target {
  rootId: string;
  path: string;
  anchor: HTMLAnchorElement;
}

interface Timers {
  open: number;
  close: number;
}

/** The first paragraph with any text, as plain text, capped. */
function excerptOf(html: string | null): string {
  if (!html) return '';
  const parsed = new DOMParser().parseFromString(html, 'text/html');
  for (const paragraph of parsed.querySelectorAll('p')) {
    const text = (paragraph.textContent ?? '').replace(/\s+/g, ' ').trim();
    if (!text) continue;
    return text.length > EXCERPT_MAX_CHARS ? `${text.slice(0, EXCERPT_MAX_CHARS).trimEnd()}…` : text;
  }
  return '';
}

/** Below the link when it fits, above otherwise; never past the viewport's right edge. */
function placeNear(anchor: DOMRect, popover: DOMRect): { left: number; top: number } {
  const maxLeft = window.innerWidth - popover.width - VIEWPORT_MARGIN_PX;
  const left = Math.max(VIEWPORT_MARGIN_PX, Math.min(anchor.left, maxLeft));
  const fitsBelow = anchor.bottom + GAP_PX + popover.height <= window.innerHeight - VIEWPORT_MARGIN_PX;
  const top = fitsBelow ? anchor.bottom + GAP_PX : Math.max(VIEWPORT_MARGIN_PX, anchor.top - GAP_PX - popover.height);
  return { left, top };
}

function useHoverTimers(setTarget: (target: Target | null) => void) {
  const timers = useRef<Timers>({ open: 0, close: 0 });

  const cancel = useCallback(() => {
    window.clearTimeout(timers.current.open);
    window.clearTimeout(timers.current.close);
  }, []);
  const scheduleOpen = useCallback(
    (target: Target) => {
      cancel();
      timers.current.open = window.setTimeout(() => setTarget(target), OPEN_DELAY_MS);
    },
    [cancel, setTarget],
  );
  const scheduleClose = useCallback(() => {
    cancel();
    timers.current.close = window.setTimeout(() => setTarget(null), CLOSE_DELAY_MS);
  }, [cancel, setTarget]);

  useEffect(() => cancel, [cancel]);
  return { cancel, scheduleOpen, scheduleClose };
}

interface HoverProps {
  containerRef: RefObject<HTMLElement | null>;
  enabled: boolean;
  resolve: (anchor: HTMLAnchorElement) => Target | null;
  scheduleOpen: (target: Target) => void;
  scheduleClose: () => void;
}

/**
 * One delegated listener per event on the prose container: the links are raw DOM
 * written by HtmlContent, so React's own handlers never see them. `mouseout` fires
 * when moving between a link's children too; those are ignored via `relatedTarget`.
 */
function useLinkHover({ containerRef, enabled, resolve, scheduleOpen, scheduleClose }: HoverProps): void {
  useEffect(() => {
    const container = containerRef.current;
    if (!container || !enabled) return;

    const anchorOf = (event: Event) => (event.target as HTMLElement | null)?.closest?.('a') ?? null;
    const onEnter = (event: Event) => {
      const anchor = anchorOf(event);
      const target = anchor ? resolve(anchor) : null;
      if (target) scheduleOpen(target);
    };
    const onLeave = (event: Event) => {
      const anchor = anchorOf(event);
      const destination = (event as MouseEvent | FocusEvent).relatedTarget as Node | null;
      if (anchor && !(destination && anchor.contains(destination))) scheduleClose();
    };

    container.addEventListener('mouseover', onEnter);
    container.addEventListener('mouseout', onLeave);
    container.addEventListener('focusin', onEnter);
    container.addEventListener('focusout', onLeave);
    return () => {
      container.removeEventListener('mouseover', onEnter);
      container.removeEventListener('mouseout', onLeave);
      container.removeEventListener('focusin', onEnter);
      container.removeEventListener('focusout', onLeave);
    };
  }, [containerRef, enabled, resolve, scheduleOpen, scheduleClose]);
}

/** Fetches the target once per page; a second hover on the same link is instant. */
function usePreview(target: Target | null): Preview | null {
  const [cache, setCache] = useState<ReadonlyMap<string, Preview>>(() => new Map());
  const key = target ? `${target.rootId} ${target.path}` : null;

  useEffect(() => {
    if (!target || !key || cache.has(key)) return;
    const controller = new AbortController();
    fetchDocument(target.rootId, target.path, controller.signal)
      .then((payload) => {
        const built = { title: payload.meta.title || payload.meta.name, excerpt: excerptOf(payload.html) };
        setCache((known) => new Map(known).set(key, built));
      })
      // A link whose target cannot be read simply shows nothing; clicking it reports why.
      .catch(() => undefined);
    return () => controller.abort();
  }, [target, key, cache]);

  return key ? (cache.get(key) ?? null) : null;
}

interface PopoverProps {
  id: string;
  target: Target;
  preview: Preview;
  onHold: () => void;
  onRelease: () => void;
}

function Popover({ id, target, preview, onHold, onRelease }: PopoverProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    const popover = ref.current;
    if (!popover) return;
    setPosition(placeNear(target.anchor.getBoundingClientRect(), popover.getBoundingClientRect()));
  }, [target, preview]);

  useEffect(() => {
    target.anchor.setAttribute('aria-describedby', id);
    return () => target.anchor.removeAttribute('aria-describedby');
  }, [target, id]);

  // Hidden until placed, so the first frame never flashes at the viewport's origin.
  const style = position ?? { left: 0, top: 0, visibility: 'hidden' as const };
  return createPortal(
    <div
      className="link-preview"
      role="tooltip"
      id={id}
      ref={ref}
      style={style}
      onMouseEnter={onHold}
      onMouseLeave={onRelease}
    >
      <p className="link-preview__title">{preview.title}</p>
      {preview.excerpt ? <p className="link-preview__excerpt">{preview.excerpt}</p> : null}
    </div>,
    document.body,
  );
}

interface LinkPreviewProps {
  containerRef: RefObject<HTMLElement | null>;
  rootId: string;
  docPath: string;
}

/**
 * A peek at where an internal link goes, on hover or focus, so a reader can decide
 * whether to follow it without losing their place. Touch has no hover: a tap navigates
 * as before and nothing here binds on a coarse pointer.
 */
export function LinkPreview({ containerRef, rootId, docPath }: LinkPreviewProps) {
  const id = useId();
  const coarsePointer = useMediaQuery('(pointer: coarse)');
  const [target, setTarget] = useState<Target | null>(null);
  const { cancel, scheduleOpen, scheduleClose } = useHoverTimers(setTarget);
  const preview = usePreview(target);

  const resolve = useCallback(
    (anchor: HTMLAnchorElement): Target | null => {
      if (anchor.target === '_blank') return null;
      const route = resolveInternalRoute(rootId, docPath, anchor.getAttribute('href') ?? '');
      const doc = route ? docRouteTarget(route) : null;
      // A link back to the open document previews what is already on screen.
      if (!doc || (doc.rootId === rootId && doc.path === docPath)) return null;
      return { ...doc, anchor };
    },
    [rootId, docPath],
  );

  useLinkHover({ containerRef, enabled: !coarsePointer, resolve, scheduleOpen, scheduleClose });

  const close = useCallback(() => {
    cancel();
    setTarget(null);
  }, [cancel]);
  useEscapeKey(target ? close : null);

  // The popover is fixed to the viewport, so once the prose scrolls it no longer sits by its link.
  useEffect(() => {
    if (!target) return;
    window.addEventListener('scroll', close, true);
    return () => window.removeEventListener('scroll', close, true);
  }, [target, close]);

  if (!target || !preview) return null;
  return <Popover id={id} target={target} preview={preview} onHold={cancel} onRelease={scheduleClose} />;
}
