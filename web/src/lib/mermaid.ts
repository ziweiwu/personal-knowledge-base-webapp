/**
 * Lazy mermaid loader.
 *
 * The library is several hundred kilobytes, so it is imported only after a
 * `<pre class="mermaid">` has actually been found in a rendered document. The
 * import lives behind a promise so a second diagram never downloads it twice.
 */
type MermaidApi = typeof import('mermaid')['default'];

let mermaidPromise: Promise<MermaidApi> | null = null;

function loadMermaid(): Promise<MermaidApi> {
  mermaidPromise ??= import('mermaid').then((module) => module.default);
  return mermaidPromise;
}

let diagramSequence = 0;

export function hasMermaid(container: HTMLElement): boolean {
  return container.querySelector('pre.mermaid') !== null;
}

/**
 * Which palette token paints which part of a diagram. Nodes sit on the accent tint
 * with an accent rule, groups on the subtle surface, notes on the warning tint, so a
 * diagram reads as part of the page in either theme instead of Mermaid's own blues.
 */
const DIAGRAM_TOKENS: Record<string, string> = {
  background: '--bg',
  textColor: '--fg',
  titleColor: '--fg',
  lineColor: '--fg-muted',
  primaryColor: '--accent-subtle',
  primaryTextColor: '--fg',
  primaryBorderColor: '--accent',
  secondaryColor: '--bg-subtle',
  secondaryTextColor: '--fg',
  secondaryBorderColor: '--border-strong',
  tertiaryColor: '--bg-inset',
  tertiaryTextColor: '--fg',
  tertiaryBorderColor: '--border-strong',
  mainBkg: '--accent-subtle',
  nodeBorder: '--accent',
  nodeTextColor: '--fg',
  clusterBkg: '--bg-subtle',
  clusterBorder: '--border-strong',
  edgeLabelBackground: '--bg',
  noteBkgColor: '--warning-subtle',
  noteTextColor: '--fg',
  noteBorderColor: '--warning-border',
  actorBkg: '--bg-subtle',
  actorBorder: '--border-strong',
  actorTextColor: '--fg',
  actorLineColor: '--fg-faint',
  signalColor: '--fg-muted',
  signalTextColor: '--fg',
  labelBoxBkgColor: '--bg-subtle',
  labelBoxBorderColor: '--border-strong',
  labelTextColor: '--fg',
  loopTextColor: '--fg',
  activationBkgColor: '--accent-subtle',
  activationBorderColor: '--accent',
  sequenceNumberColor: '--fg-on-accent',
};

/**
 * Mermaid paints into an SVG it lays out itself, from colour values it darkens and
 * lightens with its own arithmetic, so it cannot take `var(--fg)`; it needs the
 * resolved colours. This is the one place the app reads a token's computed value,
 * and it reads them fresh on every render, which is how a theme switch reaches the
 * diagrams: `renderMermaid` re-runs, and the tokens have changed underneath it.
 */
function diagramThemeVariables(): Record<string, string> {
  const tokens = getComputedStyle(document.documentElement);
  const variables: Record<string, string> = {};
  for (const [variable, token] of Object.entries(DIAGRAM_TOKENS)) {
    const value = tokens.getPropertyValue(token).trim();
    if (value) variables[variable] = value;
  }
  return variables;
}

/**
 * Replaces every mermaid source block inside the container with rendered SVG.
 *
 * The container is looked up through a callback rather than captured up front:
 * React can re-create the rendered HTML while the library download is still in
 * flight, and writing into the detached copy would silently render nothing.
 * Each diagram is rendered on its own so one bad definition cannot blank the
 * others; a failure is shown in place as an error block.
 */
export async function renderMermaid(
  getContainer: () => HTMLElement | null,
  theme: 'light' | 'dark',
  isCancelled: () => boolean,
): Promise<void> {
  const initial = getContainer();
  if (!initial || !hasMermaid(initial)) return;

  const mermaid = await loadMermaid();
  if (isCancelled()) return;

  // `base` is the one Mermaid theme built to be coloured from outside; `darkMode`
  // tells its derived shades which way to lean.
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    theme: 'base',
    darkMode: theme === 'dark',
    themeVariables: diagramThemeVariables(),
    fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--font-ui') || 'sans-serif',
  });

  for (const node of Array.from(getContainer()?.querySelectorAll<HTMLElement>('pre.mermaid') ?? [])) {
    if (!node.isConnected) continue;

    // The first render destroys the source text, so keep a copy for re-renders
    // (a theme switch re-runs this whole function).
    node.dataset.mermaidSource ??= node.textContent ?? '';
    const source = node.dataset.mermaidSource;
    if (!source.trim()) continue;

    diagramSequence += 1;
    const drawn = await drawDiagram(mermaid, `kbviewer-mermaid-${diagramSequence}`, source);
    if (isCancelled() || !node.isConnected) return;
    showDiagram(node, source, drawn);
  }
}

type Drawn = { svg: string } | { failure: unknown };

async function drawDiagram(mermaid: MermaidApi, id: string, source: string): Promise<Drawn> {
  try {
    const { svg } = await mermaid.render(id, source);
    return { svg };
  } catch (failure) {
    return { failure };
  }
}

function showDiagram(node: HTMLElement, source: string, drawn: Drawn): void {
  if ('svg' in drawn) {
    node.innerHTML = drawn.svg;
    node.dataset.mermaidState = 'ok';
    return;
  }
  node.textContent = `Diagram could not be rendered.\n\n${source}`;
  node.dataset.mermaidState = 'error';
  console.warn('[kbviewer] mermaid render failed', drawn.failure);
}
