import { fileUrl } from '../../api/paths';
import { formatDateTime, formatSize, kindLabel } from '../../lib/format';
import type { ViewerProps } from './viewer-types';

export function BinaryViewer({ payload, rootId }: ViewerProps) {
  const { meta } = payload;
  const href = fileUrl(rootId, meta.path);

  return (
    <div className="doc__inner">
      <div className="download-card">
        <span aria-hidden="true" style={{ fontSize: '2rem' }}>
          📦
        </span>
        <p className="download-card__name">{meta.name}</p>
        <p className="download-card__meta">
          {kindLabel(meta.kind)} · {formatSize(meta.size)} · modified {formatDateTime(meta.mtimeMs)}
        </p>
        <a className="btn btn--primary" href={href} download={meta.name}>
          Download file
        </a>
      </div>
    </div>
  );
}
