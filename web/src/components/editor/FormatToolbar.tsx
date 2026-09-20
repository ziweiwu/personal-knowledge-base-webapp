import { useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from 'react';
import { Button } from '../ui/Button';
import {
  cycleHeading,
  insertLink,
  toggleBold,
  toggleBullet,
  toggleInlineCode,
  toggleItalic,
  toggleTask,
  toggleWikilink,
  type FormatCommand,
} from './formatting';
import { Icon } from '../ui/Icon';

interface Tool {
  label: string;
  glyph: ReactNode;
  glyphClass?: string;
  shortcut?: string;
  command: FormatCommand;
}

const TOOLS: Tool[] = [
  { label: 'Bold', glyph: 'B', glyphClass: 'format-bar__glyph--bold', shortcut: 'Mod-B', command: toggleBold },
  { label: 'Italic', glyph: 'I', glyphClass: 'format-bar__glyph--italic', shortcut: 'Mod-I', command: toggleItalic },
  { label: 'Code', glyph: '<>', glyphClass: 'format-bar__glyph--mono', command: toggleInlineCode },
  { label: 'Link', glyph: '[ ]( )', glyphClass: 'format-bar__glyph--mono', shortcut: 'Mod-K', command: insertLink },
  { label: 'Wikilink', glyph: '[[ ]]', glyphClass: 'format-bar__glyph--mono', command: toggleWikilink },
  { label: 'Heading', glyph: 'H', glyphClass: 'format-bar__glyph--bold', command: cycleHeading },
  { label: 'Checkbox', glyph: <Icon name="check-square" size="md" />, shortcut: 'Mod-Shift-C', command: toggleTask },
  { label: 'List', glyph: <Icon name="list" size="md" />, command: toggleBullet },
];

const KEY_STEP: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1 };

/** Where Home/End land, or the neighbour in the arrow's direction, wrapping at the ends. */
function nextToolIndex(key: string, current: number): number | null {
  if (key === 'Home') return 0;
  if (key === 'End') return TOOLS.length - 1;
  const step = KEY_STEP[key];
  if (step === undefined) return null;
  return (current + step + TOOLS.length) % TOOLS.length;
}

interface FormatToolbarProps {
  /** Runs a command against the live editor; the toolbar never holds the view itself. */
  onRun: (command: FormatCommand) => void;
}

/**
 * One row of markdown marks for phone keyboards, where `**` and `[[` are three taps
 * each. Buttons never take focus from the editor: mousedown is cancelled and the command
 * puts focus back, so the selection being formatted is still the one on screen. The row
 * is one tab stop, with arrow keys moving between tools.
 */
export function FormatToolbar({ onRun }: FormatToolbarProps) {
  const [focused, setFocused] = useState(0);
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const next = nextToolIndex(event.key, focused);
    if (next === null) return;
    event.preventDefault();
    setFocused(next);
    buttons.current[next]?.focus();
  };

  const keepEditorFocus = (event: MouseEvent) => event.preventDefault();

  return (
    <div className="format-bar" role="toolbar" aria-label="Formatting" onKeyDown={onKeyDown}>
      {TOOLS.map((tool, index) => (
        <Button
          key={tool.label}
          variant="icon"
          className="format-bar__tool"
          aria-label={tool.label}
          title={tool.shortcut ? `${tool.label} (${tool.shortcut.replace('Mod', '⌘')})` : tool.label}
          tabIndex={index === focused ? 0 : -1}
          ref={(node) => {
            buttons.current[index] = node;
          }}
          onMouseDown={keepEditorFocus}
          onClick={() => onRun(tool.command)}
        >
          <span aria-hidden="true" className={tool.glyphClass}>
            {tool.glyph}
          </span>
        </Button>
      ))}
    </div>
  );
}
