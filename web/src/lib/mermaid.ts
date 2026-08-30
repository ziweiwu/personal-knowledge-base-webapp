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

  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    theme: theme === 'dark' ? 'dark' : 'default',
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
