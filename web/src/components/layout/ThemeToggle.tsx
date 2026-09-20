import { useTheme, type ThemeChoice } from '../../state/theme-context';
import { Button } from '../ui/Button';
import { Icon, type IconName } from '../ui/Icon';

const ORDER: ThemeChoice[] = ['system', 'light', 'dark'];
const ICONS: Record<ThemeChoice, IconName> = { system: 'monitor', light: 'sun', dark: 'moon' };
const LABELS: Record<ThemeChoice, string> = { system: 'System theme', light: 'Light theme', dark: 'Dark theme' };

export function ThemeToggle() {
  const { choice, setChoice } = useTheme();
  const next = ORDER[(ORDER.indexOf(choice) + 1) % ORDER.length];

  return (
    <Button
      variant="icon"
      onClick={() => setChoice(next)}
      aria-label={`${LABELS[choice]}. Switch to ${LABELS[next].toLowerCase()}`}
      title={LABELS[choice]}
    >
      <Icon name={ICONS[choice]} size="md" />
    </Button>
  );
}
