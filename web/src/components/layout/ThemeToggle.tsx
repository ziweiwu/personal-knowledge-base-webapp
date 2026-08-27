import { useTheme, type ThemeChoice } from '../../state/theme-context';

const ORDER: ThemeChoice[] = ['system', 'light', 'dark'];
const ICONS: Record<ThemeChoice, string> = { system: '🖥️', light: '☀️', dark: '🌙' };
const LABELS: Record<ThemeChoice, string> = { system: 'System theme', light: 'Light theme', dark: 'Dark theme' };

export function ThemeToggle() {
  const { choice, setChoice } = useTheme();
  const next = ORDER[(ORDER.indexOf(choice) + 1) % ORDER.length];

  return (
    <button
      type="button"
      className="btn btn--icon"
      onClick={() => setChoice(next)}
      aria-label={`${LABELS[choice]}. Switch to ${LABELS[next].toLowerCase()}`}
      title={LABELS[choice]}
    >
      <span aria-hidden="true">{ICONS[choice]}</span>
    </button>
  );
}
