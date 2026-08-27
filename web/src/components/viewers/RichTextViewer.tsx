import { HtmlContent } from '../content/HtmlContent';
import { LinkRefs } from '../content/LinkRefs';
import { TableOfContents } from '../content/TableOfContents';
import { EmptyState } from '../ui/States';
import type { ViewerProps } from './viewer-types';

/** Markdown and Word documents: both arrive as server-rendered HTML. */
export function RichTextViewer({ payload, rootId }: ViewerProps) {
  const { meta, html, headings, backlinks } = payload;

  if (html === null) {
    return <EmptyState title="Nothing to show" detail="The server returned no rendered content for this document." />;
  }

  return (
    <div className="doc__layout">
      <div className="doc__inner">
        <TableOfContents headings={headings} variant="inline" />
        <HtmlContent html={html} rootId={rootId} docPath={meta.path} />
        <LinkRefs
          title="Backlinks"
          refs={backlinks}
          rootId={rootId}
          emptyLabel="No other document links here yet."
        />
      </div>
      <TableOfContents headings={headings} variant="rail" />
    </div>
  );
}
