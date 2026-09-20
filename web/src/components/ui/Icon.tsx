import type { SVGProps } from 'react';

/**
 * One stroke-based set on a 24-unit grid, drawn in `currentColor` so every icon follows
 * the theme and the text it sits beside. Each entry is the `d` of a single path; a few
 * carry two subpaths. Emoji were replaced because the OS picks their weight and colour.
 */
const PATHS = {
  note: 'M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9zM14 3v6h6M9 13h6M9 17h6',
  document: 'M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9zM14 3v6h6M8 12h8M8 16h8',
  text: 'M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9zM14 3v6h6M8 12h8M8 15h8M8 18h5',
  pdf: 'M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9zM14 3v6h6M9 18v-6h2.5a1.5 1.5 0 0 1 0 3H9',
  code: 'M8 8l-4 4 4 4M16 8l4 4-4 4M14 5l-4 14',
  image:
    'M5 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zM3 16l5-5 4 4 3-3 6 6M15 8.5a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0',
  table: 'M3 5h18v14H3zM3 10h18M3 15h18M9 5v14M15 5v14',
  archive: 'M3 8h18v12H3zM3 4h18v4H3zM10 12h4',
  audio: 'M9 18V5l12-2v13M6 18a3 3 0 1 0 6 0 3 3 0 0 0-6 0M15 16a3 3 0 1 0 6 0 3 3 0 0 0-6 0',
  video: 'M3 6h13v12H3zM16 10l5-3v10l-5-3',
  unknown:
    'M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9zM14 3v6h6M12 17v.01M9.5 11a2.5 2.5 0 1 1 3.5 2.3c-.7.3-1 .8-1 1.7',
  folder: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
  'folder-open': 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v2M3 7v11a2 2 0 0 0 2 2h13l3-9H7l-4 9',
  'folder-plus': 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM12 10v6M9 13h6',
  'folder-move': 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM9 13h6M13 10l3 3-3 3',
  search: 'M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4',
  menu: 'M4 7h16M4 12h16M4 17h16',
  close: 'M6 6l12 12M18 6 6 18',
  sun: 'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zM12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4',
  moon: 'M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z',
  monitor: 'M4 4h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM8 20h8M12 16v4',
  expand: 'M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7',
  collapse: 'M4 14h6v6M20 10h-6V4M14 10l7-7M3 21l7-7',
  'chevron-right': 'M9 6l6 6-6 6',
  'chevron-down': 'M6 9l6 6 6-6',
  'chevron-up': 'M6 15l6-6 6 6',
  more: 'M4 12a1 1 0 1 0 2 0 1 1 0 0 0-2 0M11 12a1 1 0 1 0 2 0 1 1 0 0 0-2 0M18 12a1 1 0 1 0 2 0 1 1 0 0 0-2 0',
  edit: 'M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z',
  save: 'M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2zM17 21v-8H7v8M7 3v5h8',
  trash: 'M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6M10 11v6M14 11v6',
  restore: 'M3 12a9 9 0 1 0 3-6.7M3 4v5h5',
  rename: 'M10 4h4M12 4v16M10 20h4M4 9h5M15 9h5M4 15h5M15 15h5',
  plus: 'M12 5v14M5 12h14',
  'file-plus': 'M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9zM14 3v6h6M12 12v6M9 15h6',
  upload: 'M12 16V4M6 10l6-6 6 6M4 20h16',
  download: 'M12 4v12M6 10l6 6 6-6M4 20h16',
  print: 'M6 9V3h12v6M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2M6 14h12v7H6z',
  pin: 'M12 17v5M9 3h6l-1 6 3 3v2H7v-2l3-3z',
  clock: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 7v5l3 2',
  home: 'M3 11l9-8 9 8M5 10v10h5v-6h4v6h5V10',
  link: 'M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1.5 1.5M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1.5-1.5',
  'external-link': 'M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5',
  list: 'M9 6h12M9 12h12M9 18h12M4 6a1 1 0 1 0 2 0 1 1 0 0 0-2 0M4 12a1 1 0 1 0 2 0 1 1 0 0 0-2 0M4 18a1 1 0 1 0 2 0 1 1 0 0 0-2 0',
  check: 'M5 12l5 5 9-10',
  square: 'M5 5h14v14H5z',
  'check-square': 'M5 5h14v14H5zM8 12l3 3 5-6',
  warning: 'M12 3l10 18H2zM12 10v4M12 17v.01',
  'alert-circle': 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 8v5M12 16v.01',
  info: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v5M12 8v.01',
  inbox: 'M3 13l3-8h12l3 8v6H3zM3 13h5l2 3h4l2-3h5',
  compass: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM15.5 8.5l-2 5-5 2 2-5z',
  bulb: 'M9 18h6M10 21h4M8.5 14a5.5 5.5 0 1 1 7 0c-.9.7-1.5 1.6-1.5 2.5v.5h-4v-.5c0-.9-.6-1.8-1.5-2.5z',
  'arrow-up': 'M12 19V5M6 11l6-6 6 6',
  'arrow-down': 'M12 5v14M6 13l6 6 6-6',
  'arrow-left': 'M19 12H5M11 6l-6 6 6 6',
  'arrows-vertical': 'M8 4v16M5 7l3-3 3 3M16 20V4M13 17l3 3 3-3',
} as const;

export type IconName = keyof typeof PATHS;

interface IconProps extends Omit<SVGProps<SVGSVGElement>, 'name'> {
  name: IconName;
  /** 16px inline with text, 20px in toolbars and buttons, 28px as the one mark on an empty screen. */
  size?: 'sm' | 'md' | 'lg';
  /** Given only when the icon is the whole meaning; decorative icons stay hidden. */
  label?: string;
}

/** Pass as `className` to fill the shape too: a pressed pin, a ticked box. */
export const ICON_FILLED = 'icon--filled';

/** Thin enough to read as a mark beside text, thick enough to survive a phone's pixel grid. */
const STROKE_WIDTH = 1.75;

export function Icon({ name, size = 'sm', label, className, ...rest }: IconProps) {
  const classes = ['icon', `icon--${size}`, className].filter(Boolean).join(' ');
  return (
    <svg
      className={classes}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={STROKE_WIDTH}
      strokeLinecap="round"
      strokeLinejoin="round"
      role={label ? 'img' : undefined}
      aria-hidden={label ? undefined : true}
      focusable="false"
      {...rest}
    >
      {label ? <title>{label}</title> : null}
      <path d={PATHS[name]} />
    </svg>
  );
}
