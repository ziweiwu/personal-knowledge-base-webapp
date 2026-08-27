import type { DocumentPayload } from '../../api/types';
import type { TaskToggleHandler } from '../content/HtmlContent';

export interface ViewerProps {
  payload: DocumentPayload;
  rootId: string;
  /**
   * Present only when the document can be written. Viewers that render task lists pass it
   * to `HtmlContent`; the rest ignore it.
   */
  onToggleTask?: TaskToggleHandler;
}
