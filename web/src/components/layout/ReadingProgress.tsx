import type { RefObject } from 'react';
import { useReadingProgress } from '../../hooks/useReadingProgress';

const PERCENT = 100;

/**
 * A thin line along the bottom of the topbar showing how far the note has been read.
 * On a phone the pane's scrollbar never shows, so this is the only such cue.
 */
export function ReadingProgress({ containerRef }: { containerRef: RefObject<HTMLElement | null> }) {
  const ratio = useReadingProgress(containerRef);
  return (
    <div
      className="reading-progress"
      role="progressbar"
      aria-label="Reading progress"
      aria-valuemin={0}
      aria-valuemax={PERCENT}
      aria-valuenow={Math.round(ratio * PERCENT)}
    >
      <div className="reading-progress__fill" style={{ transform: `scaleX(${ratio})` }} />
    </div>
  );
}
