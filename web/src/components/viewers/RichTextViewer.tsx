import { HtmlContent } from '../content/HtmlContent';
import { LinkRefs } from '../content/LinkRefs';
import { TableOfContents } from '../content/TableOfContents';
import { EmptyState } from '../ui/States';
import type { ViewerProps } from './viewer-types';

/** Markdown and Word documents: both arrive as server-rendered HTML. */
export function RichTextViewer({ payload, rootId, onToggleTask }: ViewerProps) {
  const { meta, html, headings, backlinks, outlinks } = payload;

  if (html === null) {
    return <EmptyState title="Nothing to show" detail="The server returned no rendered content for this document." />;
  }

  return (
    <div className="doc__layout">
      <div className="doc__inner">
        <TableOfContents key={meta.path} headings={headings} variant="inline" />
        <HtmlContent html={html} rootId={rootId} docPath={meta.path} onToggleTask={onToggleTask} />
        <LinkRefs title="Links from this note" refs={outlinks} rootId={rootId} />
        <LinkRefs
          title="Backlinks"
          refs={backlinks}
          rootId={rootId}
          subject={{ path: meta.path, title: meta.title }}
          emptyLabel="No other document links here yet."
        />
      </div>
      <TableOfContents key={meta.path} headings={headings} variant="rail" />
    </div>
  );
}
