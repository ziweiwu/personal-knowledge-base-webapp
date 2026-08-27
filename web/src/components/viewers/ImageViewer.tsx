import { useState } from 'react';
import { createPortal } from 'react-dom';
import { fileUrl } from '../../api/paths';
import { useEscapeKey } from '../../hooks/useEscapeKey';
import { formatSize } from '../../lib/format';
import type { ViewerProps } from './viewer-types';

export function ImageViewer({ payload, rootId }: ViewerProps) {
  const [zoomed, setZoomed] = useState(false);
  const src = fileUrl(rootId, payload.meta.path);
  useEscapeKey(zoomed ? () => setZoomed(false) : null);

  return (
    <div className="doc__inner">
      <button
        type="button"
        className="image-viewer__button"
        onClick={() => setZoomed(true)}
        aria-label={`View ${payload.meta.name} at full size`}
      >
        <img className="image-viewer__img" src={src} alt={payload.meta.title || payload.meta.name} />
      </button>
      <p className="doc__meta" style={{ marginTop: 8 }}>
        {payload.meta.name} · {formatSize(payload.meta.size)} ·{' '}
        <a href={src} download={payload.meta.name}>
          Download
        </a>
      </p>

      {zoomed
        ? createPortal(
            <div
              className="lightbox"
              role="dialog"
              aria-modal="true"
              aria-label={`${payload.meta.name}, full size`}
              onClick={() => setZoomed(false)}
            >
              <img src={src} alt={payload.meta.title || payload.meta.name} />
              <button
                type="button"
                className="btn"
                style={{ position: 'fixed', top: 12, right: 12 }}
                onClick={() => setZoomed(false)}
                autoFocus
              >
                Close
              </button>
            </div>,
            document.body,
          )
        : null}
    </div>
  );
}
