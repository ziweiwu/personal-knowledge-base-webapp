import { fileUrl } from '../../api/paths';
import { formatSize } from '../../lib/format';
import type { ViewerProps } from './viewer-types';

/**
 * iOS Safari renders embedded PDFs as a single unusable page, and gives no error
 * event when it does. The "open in a new tab" escape hatch is therefore always
 * on screen rather than being shown only after a failure.
 */
export function PdfViewer({ payload, rootId }: ViewerProps) {
  const href = fileUrl(rootId, payload.meta.path);

  return (
    <div className="doc__inner pdf-viewer">
      <div className="pdf-viewer__fallback">
        <a className="btn btn--primary" href={href} target="_blank" rel="noopener noreferrer">
          Open PDF in a new tab
        </a>
        <a className="btn" href={href} download={payload.meta.name}>
          Download
        </a>
        <span>
          {payload.meta.name} · {formatSize(payload.meta.size)}
        </span>
      </div>
      <object className="pdf-viewer__frame" data={href} type="application/pdf" aria-label={`PDF preview of ${payload.meta.name}`}>
        <div className="state">
          <p className="state__title">This browser cannot display PDFs inline</p>
          <p className="state__detail">Use the buttons above to open or download the file.</p>
        </div>
      </object>
    </div>
  );
}
