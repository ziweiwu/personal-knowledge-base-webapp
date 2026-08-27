import type {
  ApiError,
  DocumentMeta,
  DocumentPayload,
  FolderListing,
  RenameRequest,
  RenameResult,
  RootInfo,
  SaveConflict,
  SaveRequest,
  SearchHit,
  SessionInfo,
  TaskToggleRequest,
  TreeNode,
} from './types';
import { API_BASE, docUrl, fileUrl, folderUrl, rawUrl, resourceUrl } from './paths';
import {
  HTTP_CONFLICT,
  HTTP_NO_CONTENT,
  HTTP_NOT_FOUND,
  HTTP_TOO_MANY_REQUESTS,
  HTTP_UNAUTHORIZED,
} from './status';

/**
 * Identifies this browser tab so the SSE stream's `origin` field can be used to
 * ignore the echo of our own writes.
 *
 * The header name is not pinned down by the API contract; the server is free to
 * ignore it, in which case we simply refetch after our own saves too.
 */
export const CLIENT_ORIGIN = `web-${Math.random().toString(36).slice(2, 10)}`;
const ORIGIN_HEADER = 'X-Kbview-Origin';

/** A non-JSON error body is shown to the user, so only its opening is kept. */
const MAX_ERROR_MESSAGE_CHARS = 400;

export class ApiRequestError extends Error {
  readonly status: number;
  readonly code: string;
  readonly body: unknown;

  constructor(status: number, code: string, message: string, body: unknown) {
    super(message);
    this.name = 'ApiRequestError';
    this.status = status;
    this.code = code;
    this.body = body;
  }

  get isUnauthorized(): boolean {
    return this.status === HTTP_UNAUTHORIZED;
  }

  get isNotFound(): boolean {
    return this.status === HTTP_NOT_FOUND;
  }

  get isConflict(): boolean {
    return this.status === HTTP_CONFLICT;
  }

  get isRateLimited(): boolean {
    return this.status === HTTP_TOO_MANY_REQUESTS;
  }
}

/** A save rejected with 409 because the file changed underneath us. */
export class SaveConflictError extends ApiRequestError {
  readonly conflict: SaveConflict;

  constructor(conflict: SaveConflict) {
    super(HTTP_CONFLICT, 'conflict', 'The file changed on disk since it was opened.', conflict);
    this.name = 'SaveConflictError';
    this.conflict = conflict;
  }
}

type Transport = (input: string, init?: RequestInit) => Promise<Response>;

let transport: Transport = (input, init) => fetch(input, init);

/** Used only by the development mock; the production path never calls this. */
export function setTransport(next: Transport): void {
  transport = next;
}

type UnauthorizedListener = () => void;
const unauthorizedListeners = new Set<UnauthorizedListener>();

/**
 * Any 401 anywhere means the session is gone; the app listens here and sends the
 * user to /login rather than leaving a half-broken screen behind.
 */
export function onUnauthorized(listener: UnauthorizedListener): () => void {
  unauthorizedListeners.add(listener);
  return () => unauthorizedListeners.delete(listener);
}

/** Login's own 401 is an answer, not an expired session, so it opts out. */
let suppressUnauthorizedBroadcast = 0;

function broadcastUnauthorized(): void {
  if (suppressUnauthorizedBroadcast > 0) return;
  for (const listener of unauthorizedListeners) listener();
}

async function readErrorBody(response: Response): Promise<{ code: string; message: string; body: unknown }> {
  const fallback = { code: `http_${response.status}`, message: response.statusText || 'Request failed', body: null };
  const contentType = response.headers.get('content-type') ?? '';
  if (!contentType.includes('json')) {
    const text = await response.text().catch(() => '');
    return text ? { ...fallback, message: text.slice(0, MAX_ERROR_MESSAGE_CHARS), body: text } : fallback;
  }
  const parsed = (await response.json().catch(() => null)) as ApiError | null;
  if (parsed && typeof parsed.message === 'string') {
    return { code: parsed.error ?? fallback.code, message: parsed.message, body: parsed };
  }
  return { ...fallback, body: parsed };
}

interface RequestOptions {
  method?: string;
  json?: unknown;
  body?: BodyInit;
  headers?: Record<string, string>;
  signal?: AbortSignal;
  /** Mutating calls tag themselves so the SSE echo can be filtered out. */
  mutating?: boolean;
}

async function request(url: string, options: RequestOptions = {}): Promise<Response> {
  const headers: Record<string, string> = { Accept: 'application/json', ...options.headers };
  let body = options.body;
  if (options.json !== undefined) {
    headers['Content-Type'] = 'application/json';
    body = JSON.stringify(options.json);
  }
  if (options.mutating) {
    headers[ORIGIN_HEADER] = CLIENT_ORIGIN;
  }

  const init: RequestInit = {
    method: options.method ?? 'GET',
    headers,
    credentials: 'same-origin',
    ...(body === undefined ? {} : { body }),
    ...(options.signal ? { signal: options.signal } : {}),
  };

  const response = await transport(url, init);
  if (response.ok) return response;

  if (response.status === HTTP_UNAUTHORIZED) {
    broadcastUnauthorized();
  }
  const { code, message, body: errorBody } = await readErrorBody(response);
  throw new ApiRequestError(response.status, code, message, errorBody);
}

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const response = await request(url, signal ? { signal } : {});
  return (await response.json()) as T;
}

/* ------------------------------------------------------------------ auth -- */

export async function login(email: string, password: string): Promise<void> {
  suppressUnauthorizedBroadcast += 1;
  try {
    await request(`${API_BASE}/auth/login`, { method: 'POST', json: { email, password } });
  } finally {
    suppressUnauthorizedBroadcast -= 1;
  }
}

export async function logout(): Promise<void> {
  await request(`${API_BASE}/auth/logout`, { method: 'POST', mutating: true });
}

export async function fetchSession(signal?: AbortSignal): Promise<SessionInfo> {
  suppressUnauthorizedBroadcast += 1;
  try {
    return await getJson<SessionInfo>(`${API_BASE}/auth/session`, signal);
  } finally {
    suppressUnauthorizedBroadcast -= 1;
  }
}

/* --------------------------------------------------------------- reading -- */

export function fetchRoots(signal?: AbortSignal): Promise<RootInfo[]> {
  return getJson<RootInfo[]>(`${API_BASE}/roots`, signal);
}

export function fetchTree(rootId: string, signal?: AbortSignal): Promise<TreeNode[]> {
  return getJson<TreeNode[]>(`${API_BASE}/tree?root=${encodeURIComponent(rootId)}`, signal);
}

export function fetchFolder(rootId: string, path: string, signal?: AbortSignal): Promise<FolderListing> {
  return getJson<FolderListing>(folderUrl(rootId, path), signal);
}

export function fetchDocument(rootId: string, path: string, signal?: AbortSignal): Promise<DocumentPayload> {
  return getJson<DocumentPayload>(docUrl(rootId, path), signal);
}

export async function fetchRaw(rootId: string, path: string, signal?: AbortSignal): Promise<string> {
  const response = await request(rawUrl(rootId, path), {
    headers: { Accept: 'text/plain' },
    ...(signal ? { signal } : {}),
  });
  return response.text();
}

export function search(rootId: string, query: string, signal?: AbortSignal): Promise<SearchHit[]> {
  const url = `${API_BASE}/search?root=${encodeURIComponent(rootId)}&q=${encodeURIComponent(query)}`;
  return getJson<SearchHit[]>(url, signal);
}

/* --------------------------------------------------------------- writing -- */

function looksLikeSaveConflict(value: unknown): value is SaveConflict {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<SaveConflict>;
  return typeof candidate.diskContent === 'string' && typeof candidate.diskMtimeMs === 'number';
}

/**
 * Optimistic-concurrency save. A 409 carries both versions, and is surfaced as a
 * `SaveConflictError` so the caller can offer a choice instead of overwriting.
 */
export async function saveDocument(
  rootId: string,
  path: string,
  payload: SaveRequest,
): Promise<DocumentMeta> {
  try {
    const response = await request(docUrl(rootId, path), { method: 'PUT', json: payload, mutating: true });
    return (await response.json()) as DocumentMeta;
  } catch (error) {
    if (error instanceof ApiRequestError && error.isConflict && looksLikeSaveConflict(error.body)) {
      throw new SaveConflictError(error.body);
    }
    throw error;
  }
}

export async function createDocument(rootId: string, path: string, content: string): Promise<DocumentMeta | null> {
  const response = await request(docUrl(rootId, path), {
    method: 'POST',
    json: { content, baseMtimeMs: 0 } satisfies SaveRequest,
    mutating: true,
  });
  if (response.status === HTTP_NO_CONTENT) return null;
  return (await response.json().catch(() => null)) as DocumentMeta | null;
}

/** Every document carrying `tag`, including nested tags beneath it. */
export async function fetchTagged(rootId: string, tag: string, signal?: AbortSignal): Promise<DocumentMeta[]> {
  return getJson<DocumentMeta[]>(resourceUrl('tag', rootId, tag), signal);
}

/**
 * Tick or untick one task-list checkbox.
 *
 * Carries `baseMtimeMs` exactly as a save does: a checkbox is a smaller edit, not a
 * licence to overwrite someone else's. Returns the document's new meta so the caller can
 * keep its concurrency token current for the next tick.
 */
export async function toggleTask(
  rootId: string,
  path: string,
  toggle: TaskToggleRequest,
): Promise<DocumentMeta> {
  const response = await request(resourceUrl('task', rootId, path), {
    method: 'POST',
    json: toggle,
    mutating: true,
  });
  return (await response.json()) as DocumentMeta;
}

export async function createFolder(rootId: string, path: string): Promise<void> {
  await request(folderUrl(rootId, path), { method: 'POST', mutating: true });
}

/** Moves the document to `.trash/` server-side rather than unlinking it. */
export async function deleteDocument(rootId: string, path: string): Promise<void> {
  await request(docUrl(rootId, path), { method: 'DELETE', mutating: true });
}

export async function renameDocument(rootId: string, payload: RenameRequest): Promise<RenameResult> {
  const url = `${API_BASE}/rename?root=${encodeURIComponent(rootId)}`;
  const response = await request(url, { method: 'POST', json: payload, mutating: true });
  return (await response.json()) as RenameResult;
}

/**
 * Uploads a file's bytes. `POST` to the file route takes raw bytes, which keeps binary
 * content off the JSON document-create route.
 */
export async function uploadFile(rootId: string, path: string, file: File): Promise<void> {
  await request(fileUrl(rootId, path), {
    method: 'POST',
    body: file,
    headers: { 'Content-Type': file.type || 'application/octet-stream' },
    mutating: true,
  });
}
