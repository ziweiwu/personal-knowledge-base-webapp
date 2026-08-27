import { createElement, type ComponentType } from 'react';
import type { DocumentKind } from '../../api/types';
import { BinaryViewer } from './BinaryViewer';
import { CsvViewer } from './CsvViewer';
import { ImageViewer } from './ImageViewer';
import { PdfViewer } from './PdfViewer';
import { RichTextViewer } from './RichTextViewer';
import { TextViewer } from './TextViewer';
import type { ViewerProps } from './viewer-types';

/**
 * The one place a document kind becomes a component. Adding a variant to the
 * Rust `DocumentKind` enum makes this map fail to typecheck until it is handled.
 */
const VIEWERS: Record<DocumentKind, ComponentType<ViewerProps>> = {
  markdown: RichTextViewer,
  docx: RichTextViewer,
  pdf: PdfViewer,
  image: ImageViewer,
  csv: CsvViewer,
  text: TextViewer,
  binary: BinaryViewer,
};

export function DocumentBody({ payload, rootId, onToggleTask }: ViewerProps) {
  return createElement(VIEWERS[payload.meta.kind] ?? BinaryViewer, { payload, rootId, onToggleTask });
}
