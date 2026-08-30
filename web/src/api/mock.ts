/**
 * Development-only fixture backend.
 *
 * Loaded exclusively from `main.tsx` behind `VITE_MOCK=1`, via a dynamic import,
 * so it is never part of a production bundle. It answers the documented routes
 * only — the application code above it has no idea the mock exists.
 */
import { setTransport } from './client';
import {
  HTTP_CONFLICT,
  HTTP_CREATED,
  HTTP_INTERNAL_ERROR,
  HTTP_NO_CONTENT,
  HTTP_NOT_FOUND,
  HTTP_OK,
  HTTP_TOO_MANY_REQUESTS,
  HTTP_UNAUTHORIZED,
} from './status';
import type {
  DocumentMeta,
  DocumentPayload,
  FolderEntry,
  FolderListing,
  RootInfo,
  SaveRequest,
  SearchHit,
  TreeNode,
} from './types';

const NOW = Date.now();

let sessionActive = true;

interface MockFile {
  meta: DocumentMeta;
  source: string;
  html: string | null;
  headings?: DocumentPayload['headings'];
  backlinks?: DocumentPayload['backlinks'];
  renderWarning?: string;
}

const files = new Map<string, MockFile>();

function addFile(file: MockFile): void {
  files.set(file.meta.path, file);
}

addFile({
  meta: {
    path: 'index.md',
    name: 'index.md',
    title: 'Knowledge Base',
    kind: 'markdown',
    size: 820,
    mtimeMs: NOW - 90_000,
    editable: true,
    tags: ['home', 'meta'],
  },
  source: '# Knowledge Base\n\nStart here.\n',
  html:
    '<h1 id="knowledge-base">Knowledge Base</h1>' +
    '<p>Start here. See <a href="/n/kb/projects/kbviewer.md">kbviewer</a>.</p>' +
    '<h2 id="sections">Sections</h2><ul><li>Projects</li><li>Reading</li></ul>',
  headings: [
    { depth: 1, text: 'Knowledge Base', slug: 'knowledge-base' },
    { depth: 2, text: 'Sections', slug: 'sections' },
  ],
});

addFile({
  meta: {
    path: 'projects/kbviewer.md',
    name: 'kbviewer.md',
    title: 'kbviewer',
    kind: 'markdown',
    size: 2400,
    mtimeMs: NOW - 3_600_000,
    editable: true,
    tags: ['project', 'rust'],
  },
  source: '# kbviewer\n\nA browser for local folders.\n\n## Design\n\nServer renders, client displays.\n',
  html:
    '<h1 id="kbviewer">kbviewer</h1><p>A browser for local folders of documents.</p>' +
    '<h2 id="design">Design</h2><p>The server renders; the client displays. Inline math: ' +
    '<math><mrow><mi>a</mi><mo>+</mo><mi>b</mi></mrow></math>.</p>' +
    '<pre class="mermaid">graph LR\n  Browser --&gt; Server\n  Server --&gt; Disk</pre>' +
    '<h2 id="table">Table</h2><table><thead><tr><th>Route</th><th>Returns</th></tr></thead>' +
    '<tbody><tr><td>/api/doc</td><td>DocumentPayload</td></tr></tbody></table>' +
    '<h3 id="notes">Notes</h3><pre><code>cargo run -- --config kbviewer.config.json</code></pre>',
  headings: [
    { depth: 1, text: 'kbviewer', slug: 'kbviewer' },
    { depth: 2, text: 'Design', slug: 'design' },
    { depth: 2, text: 'Table', slug: 'table' },
    { depth: 3, text: 'Notes', slug: 'notes' },
  ],
  backlinks: [{ path: 'index.md', title: 'Knowledge Base' }],
  renderWarning: 'One embedded attachment could not be resolved.',
});

addFile({
  meta: {
    path: 'projects/budget.csv',
    name: 'budget.csv',
    title: 'budget',
    kind: 'csv',
    size: 210,
    mtimeMs: NOW - 7_200_000,
    editable: true,
  },
  source: 'item,cost,owner\nserver,120,ziwei\ndomain,14,ziwei\nbackup,60,shared\n',
  html:
    '<table><thead><tr><th>item</th><th>cost</th><th>owner</th></tr></thead><tbody>' +
    '<tr><td>server</td><td>120</td><td>ziwei</td></tr>' +
    '<tr><td>domain</td><td>14</td><td>ziwei</td></tr>' +
    '<tr><td>backup</td><td>60</td><td>shared</td></tr></tbody></table>',
});

addFile({
  meta: {
    path: 'papers/attention.pdf',
    name: 'attention.pdf',
    title: 'attention',
    kind: 'pdf',
    size: 1_240_000,
    mtimeMs: NOW - 86_400_000,
    editable: false,
  },
  source: '',
  html: null,
});

addFile({
  meta: {
    path: 'papers/diagram.png',
    name: 'diagram.png',
    title: 'diagram',
    kind: 'image',
    size: 48_000,
    mtimeMs: NOW - 172_800_000,
    editable: false,
  },
  source: '',
  html: null,
});

addFile({
  meta: {
    path: 'papers/archive.zip',
    name: 'archive.zip',
    title: 'archive',
    kind: 'binary',
    size: 9_400_000,
    mtimeMs: NOW - 400_000_000,
    editable: false,
  },
  source: '',
  html: null,
});

addFile({
  meta: {
    path: 'notes/todo.txt',
    name: 'todo.txt',
    title: 'todo',
    kind: 'text',
    size: 96,
    mtimeMs: NOW - 600_000,
    editable: true,
  },
  source: 'buy milk\nship kbviewer\n',
  html: '<pre><code>buy milk\nship kbviewer\n</code></pre>',
});

const roots: RootInfo[] = [
  { id: 'kb', name: 'Knowledge Base', obsidianMode: true, readOnly: false },
  { id: 'papers', name: 'Papers (read only)', obsidianMode: false, readOnly: true },
];

function buildTree(): TreeNode[] {
  const rootNodes: TreeNode[] = [];
  const dirs = new Map<string, TreeNode>();

  const ensureDir = (path: string): TreeNode => {
    const existing = dirs.get(path);
    if (existing) return existing;
    const cut = path.lastIndexOf('/');
    const node: TreeNode = { name: path.slice(cut + 1), path, isDir: true, children: [] };
    dirs.set(path, node);
    if (cut === -1) rootNodes.push(node);
    else ensureDir(path.slice(0, cut)).children?.push(node);
    return node;
  };

  for (const file of files.values()) {
    const { path, name, kind } = file.meta;
    const cut = path.lastIndexOf('/');
    const node: TreeNode = { name, path, isDir: false, kind };
    if (cut === -1) rootNodes.push(node);
    else ensureDir(path.slice(0, cut)).children?.push(node);
  }
  return rootNodes;
}

function payloadFor(file: MockFile): DocumentPayload {
  return {
    meta: { ...file.meta },
    html: file.html,
    headings: file.headings ?? [],
    backlinks: file.backlinks ?? [],
    outlinks: [],
    ...(file.renderWarning ? { renderWarning: file.renderWarning } : {}),
  };
}

function fileEntry(file: MockFile): FolderEntry {
  return {
    name: file.meta.name,
    path: file.meta.path,
    isDir: false,
    kind: file.meta.kind,
    size: file.meta.size,
    mtimeMs: file.meta.mtimeMs,
  };
}

function directoryEntry(path: string, name: string): FolderEntry {
  return { name, path, isDir: true, size: 0, mtimeMs: NOW, childCount: 2 };
}

function listFolder(path: string): FolderListing {
  const prefix = path ? `${path}/` : '';
  const seenDirs = new Set<string>();
  const entries: FolderEntry[] = [];

  for (const file of files.values()) {
    const filePath = file.meta.path;
    if (!filePath.startsWith(prefix)) continue;
    const rest = filePath.slice(prefix.length);
    const cut = rest.indexOf('/');
    if (cut === -1) {
      if (rest !== 'index.md') entries.push(fileEntry(file));
      continue;
    }
    const dirName = rest.slice(0, cut);
    if (seenDirs.has(dirName)) continue;
    seenDirs.add(dirName);
    entries.push(directoryEntry(`${prefix}${dirName}`, dirName));
  }

  const index = files.get(`${prefix}index.md`);
  return {
    path,
    name: path ? path.slice(path.lastIndexOf('/') + 1) : 'Knowledge Base',
    entries,
    ...(index ? { index: payloadFor(index) } : {}),
  };
}

function json(body: unknown, status = HTTP_OK): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

function error(status: number, code: string, message: string): Response {
  return json({ error: code, message }, status);
}

function decodeTail(segments: string[]): string {
  return segments.map(decodeURIComponent).join('/');
}

/** How much of the surrounding text is kept on either side of a search hit. */
const SNIPPET_LEAD_CHARS = 30;

function markSnippet(source: string, query: string): string {
  const at = source.toLowerCase().indexOf(query.toLowerCase());
  if (at === -1) return source.slice(0, 100);
  const from = Math.max(0, at - SNIPPET_LEAD_CHARS);
  return `${from > 0 ? '\u2026' : ''}${source.slice(from, at)}**${source.slice(at, at + query.length)}**${source.slice(
    at + query.length,
    at + query.length + 60,
  )}`;
}

/* ------------------------------------------------------------- routing -- */

/** One decoded request, in the shape the per-endpoint handlers want it. */
interface MockRequest {
  method: string;
  /** The vault-relative path, with the root id segment already stripped. */
  path: string;
  query: URLSearchParams;
  body: string;
}

type RouteHandler = (request: MockRequest) => Response;

const MOCK_PASSWORD = 'password';
const MOCK_EMAIL = 'you@example.com';
/** Signing in as this address always answers 429, so the lockout copy is reachable. */
const RATE_LIMITED_EMAIL = 'lock@example.com';

/** Returns `null` for an unknown action so the caller falls through to a 404. */
function handleAuth(action: string | undefined, request: MockRequest): Response | null {
  if (action === 'login') {
    const body = JSON.parse(request.body) as { email?: string; password?: string };
    if (body.email === RATE_LIMITED_EMAIL) return error(HTTP_TOO_MANY_REQUESTS, 'rate_limited', 'Too many attempts.');
    if (body.password !== MOCK_PASSWORD) return error(HTTP_UNAUTHORIZED, 'unauthorized', 'Invalid credentials.');
    sessionActive = true;
    return new Response(null, { status: HTTP_OK });
  }
  if (action === 'logout') {
    sessionActive = false;
    return new Response(null, { status: HTTP_NO_CONTENT });
  }
  if (action === 'session') {
    return sessionActive ? json({ email: MOCK_EMAIL }) : error(HTTP_UNAUTHORIZED, 'unauthorized', 'No session.');
  }
  return null;
}

function handleSearch(request: MockRequest): Response {
  const query = request.query.get('q') ?? '';
  if (!query.trim()) return json([]);
  const hits: SearchHit[] = [];
  for (const file of files.values()) {
    const haystack = `${file.meta.title}\n${file.source}`;
    if (!haystack.toLowerCase().includes(query.toLowerCase())) continue;
    hits.push({
      path: file.meta.path,
      title: file.meta.title,
      kind: file.meta.kind,
      snippet: markSnippet(haystack, query),
      score: 1,
    });
  }
  return json(hits);
}

function handleFolder(request: MockRequest): Response {
  if (request.method === 'POST') return new Response(null, { status: HTTP_CREATED });
  return json(listFolder(request.path));
}

function handleRename(request: MockRequest): Response {
  const body = JSON.parse(request.body) as { from: string; to: string };
  const file = files.get(body.from);
  if (file) {
    files.delete(body.from);
    file.meta = { ...file.meta, path: body.to, name: body.to.slice(body.to.lastIndexOf('/') + 1) };
    files.set(body.to, file);
  }
  return json({ from: body.from, to: body.to, updated: ['index.md', 'projects/kbviewer.md'] });
}

function createDocument(path: string, existing: MockFile | undefined): Response {
  if (existing) return error(HTTP_CONFLICT, 'exists', 'A file already exists at that path.');
  const name = path.slice(path.lastIndexOf('/') + 1);
  addFile({
    meta: {
      path,
      name,
      title: name.replace(/\.[^.]+$/, ''),
      kind: 'markdown',
      size: 0,
      mtimeMs: Date.now(),
      editable: true,
    },
    source: '',
    html: '',
  });
  return json(files.get(path)?.meta, HTTP_CREATED);
}

/** A one-in-N save is answered with a 409 so the conflict flow is exercisable. */
const CONFLICT_EVERY_NTH_SAVE = 3;
let saveCount = 0;

function conflictResponse(path: string, file: MockFile, yourContent: string): Response {
  return json(
    {
      path,
      yourContent,
      diskContent: `${file.source}\n\n<!-- edited in Obsidian at ${new Date().toISOString()} -->\n`,
      diskMtimeMs: Date.now(),
    },
    HTTP_CONFLICT,
  );
}

function saveDocument(request: MockRequest, file: MockFile | undefined): Response {
  if (!file) return error(HTTP_NOT_FOUND, 'not_found', 'No such document.');
  const body = JSON.parse(request.body) as SaveRequest;
  saveCount += 1;
  if (saveCount % CONFLICT_EVERY_NTH_SAVE === 0) return conflictResponse(request.path, file, body.content);
  file.source = body.content;
  file.html = `<pre><code>${body.content.replace(/</g, '&lt;')}</code></pre>`;
  file.meta = { ...file.meta, mtimeMs: Date.now(), size: body.content.length };
  return json(file.meta);
}

function handleDoc(request: MockRequest): Response {
  const file = files.get(request.path);
  if (request.method === 'DELETE') {
    files.delete(request.path);
    return new Response(null, { status: HTTP_NO_CONTENT });
  }
  if (request.method === 'POST') return createDocument(request.path, file);
  if (request.method === 'PUT') return saveDocument(request, file);
  if (!file) return error(HTTP_NOT_FOUND, 'not_found', 'No such document.');
  return json(payloadFor(file));
}

function handleRaw(request: MockRequest): Response {
  const file = files.get(request.path);
  if (!file) return error(HTTP_NOT_FOUND, 'not_found', 'No such document.');
  return new Response(file.source, { status: HTTP_OK, headers: { 'Content-Type': 'text/plain' } });
}

function handleFile(request: MockRequest): Response {
  if (!files.has(request.path)) return error(HTTP_NOT_FOUND, 'not_found', 'No such file.');
  return new Response(new Blob(['mock bytes']), { status: HTTP_OK });
}

const ROUTES = new Map<string, RouteHandler>([
  ['roots', () => json(roots)],
  ['tree', () => json(buildTree())],
  ['search', handleSearch],
  ['folder', handleFolder],
  ['rename', handleRename],
  ['doc', handleDoc],
  ['raw', handleRaw],
  ['file', handleFile],
]);

async function handle(url: string, init: RequestInit): Promise<Response> {
  const parsed = new URL(url, window.location.origin);
  const segments = parsed.pathname.replace(/^\/api\/?/, '').split('/').filter(Boolean);
  const [head, ...rest] = segments;
  const request: MockRequest = {
    method: (init.method ?? 'GET').toUpperCase(),
    // `rest[0]` is the root id; the mock serves every root from one file map.
    path: decodeTail(rest.slice(1)),
    query: parsed.searchParams,
    body: String(init.body ?? '{}'),
  };

  if (head === 'auth') {
    const answered = handleAuth(rest[0], request);
    if (answered) return answered;
  }

  if (!sessionActive) return error(HTTP_UNAUTHORIZED, 'unauthorized', 'No session.');

  const route = ROUTES.get(head ?? '');
  if (!route) return error(HTTP_NOT_FOUND, 'not_found', `Unhandled mock route: ${parsed.pathname}`);
  return route(request);
}

/** How long the stubbed `EventSource` waits before reporting itself open. */
const MOCK_OPEN_DELAY_MS = 30;

/**
 * `EventSource` bypasses the fetch transport, so without a stub the dev page
 * would hammer a `/api/events` endpoint that no backend is serving.
 */
function installMockEventSource(): void {
  class MockEventSource extends EventTarget {
    static readonly CONNECTING = 0;
    static readonly OPEN = 1;
    static readonly CLOSED = 2;
    readonly readyState = MockEventSource.OPEN;
    onopen: ((event: Event) => void) | null = null;
    onmessage: ((event: MessageEvent) => void) | null = null;
    onerror: ((event: Event) => void) | null = null;

    constructor() {
      super();
      setTimeout(() => this.onopen?.(new Event('open')), MOCK_OPEN_DELAY_MS);
    }

    close(): void {
      /* nothing to tear down */
    }
  }
  window.EventSource = MockEventSource as unknown as typeof EventSource;
}

/** Stands in for network latency so loading states are visible during development. */
const MOCK_LATENCY_MS = 140;

export function installMockTransport(): void {
  installMockEventSource();
  setTransport(async (url, init = {}) => {
    await new Promise((resolve) => setTimeout(resolve, MOCK_LATENCY_MS));
    try {
      return await handle(url, init);
    } catch (cause) {
      return error(HTTP_INTERNAL_ERROR, 'mock_error', cause instanceof Error ? cause.message : 'Mock failure');
    }
  });
  console.info(`[kbviewer] mock API active \u2014 sign in with any email and the password "${MOCK_PASSWORD}"`);
}
