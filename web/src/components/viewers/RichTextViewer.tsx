import { HtmlContent } from '../content/HtmlContent';
import { TableOfContents } from '../content/TableOfContents';
import { EmptyState } from '../ui/States';
import type { ViewerProps } from './viewer-types';

/** Markdown and Word documents: both arrive as server-rendered HTML. Their links are
    listed by the page, in the margin beside the text. */
export function RichTextViewer({ payload, rootId, onToggleTask }: ViewerProps) {
  const { meta, html, headings } = payload;

  if (html === null) {
    return <EmptyState title="Nothing to show" detail="The server returned no rendered content for this document." />;
  }

  return (
    <div className="doc__layout">
      <div className="doc__inner">
        <TableOfContents key={meta.path} headings={headings} variant="inline" />
        <HtmlContent html={html} rootId={rootId} docPath={meta.path} onToggleTask={onToggleTask} />
      </div>
      <TableOfContents key={meta.path} headings={headings} variant="rail" />
    </div>
  );
}
