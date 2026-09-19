import { useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { fileUrl } from '../../api/paths';
import { useBodyScrollLock } from '../../hooks/useBodyScrollLock';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useInertBackground } from '../../hooks/useInertBackground';
import { formatSize } from '../../lib/format';
import { Button } from '../ui/Button';
import type { ViewerProps } from './viewer-types';

interface LightboxProps {
  src: string;
  alt: string;
  name: string;
  onClose: () => void;
}

/**
 * Mounted only while zoomed, so the same hooks the Modal uses run for exactly
 * that span: the page behind is inert, focus stays inside, the pane does not
 * scroll, and Escape closes. Order matters: inert before the trap.
 */
function Lightbox({ src, alt, name, onClose }: LightboxProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  useInertBackground();
  useFocusTrap(panelRef);
  useEscapeKey(onClose);
  useBodyScrollLock('locked');

  return createPortal(
    <div
      className="lightbox"
      role="dialog"
      aria-modal="true"
      aria-label={`${name}, full size`}
      ref={panelRef}
      onClick={onClose}
    >
      <img src={src} alt={alt} />
      <Button className="lightbox__close" onClick={onClose} autoFocus>
        Close
      </Button>
    </div>,
    document.body,
  );
}

export function ImageViewer({ payload, rootId }: ViewerProps) {
  const [zoomed, setZoomed] = useState(false);
  const src = fileUrl(rootId, payload.meta.path);
  const alt = payload.meta.title || payload.meta.name;

  return (
    <div className="doc__inner">
      <button
        type="button"
        className="image-viewer__button"
        onClick={() => setZoomed(true)}
        aria-label={`View ${payload.meta.name} at full size`}
      >
        <img className="image-viewer__img" src={src} alt={alt} />
      </button>
      <p className="doc__meta image-viewer__meta">
        {payload.meta.name} · {formatSize(payload.meta.size)} ·{' '}
        <a href={src} download={payload.meta.name}>
          Download
        </a>
      </p>

      {zoomed ? <Lightbox src={src} alt={alt} name={payload.meta.name} onClose={() => setZoomed(false)} /> : null}
    </div>
  );
}
