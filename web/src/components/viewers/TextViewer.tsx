import { HtmlContent } from '../content/HtmlContent';
import { EmptyState } from '../ui/States';
import type { ViewerProps } from './viewer-types';

/** Plain-text files arrive already syntax-highlighted by the server. */
export function TextViewer({ payload, rootId }: ViewerProps) {
  if (payload.html === null) {
    return <EmptyState title="Nothing to show" detail="The server returned no rendered content for this file." />;
  }
  return (
    <div className="doc__inner">
      <HtmlContent html={payload.html} rootId={rootId} docPath={payload.meta.path} />
    </div>
  );
}
