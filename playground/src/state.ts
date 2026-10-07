import type { SyntaxConfig, UndefinedBehavior } from "minijinja-js";

export interface TemplateFile {
  name: string;
  source: string;
}

export interface Settings {
  pycompat: boolean;
  trimBlocks: boolean;
  lstripBlocks: boolean;
  keepTrailingNewline: boolean;
  undefinedBehavior: UndefinedBehavior;
  /** Only contains the values that differ from the defaults. */
  syntax: Partial<SyntaxConfig>;
}

/** The state that is persisted and shared via links. */
export interface PlaygroundState {
  files: TemplateFile[];
  /** The template that is rendered. */
  entry: string;
  /** The context as JSON source. */
  context: string;
  settings: Settings;
}

export const DEFAULT_SETTINGS: Settings = {
  pycompat: false,
  trimBlocks: false,
  lstripBlocks: false,
  keepTrailingNewline: false,
  undefinedBehavior: "lenient",
  syntax: {},
};

export const DEFAULT_SYNTAX: SyntaxConfig = {
  blockStart: "{%",
  blockEnd: "%}",
  variableStart: "{{",
  variableEnd: "}}",
  commentStart: "{#",
  commentEnd: "#}",
  lineStatementPrefix: null,
  lineCommentPrefix: null,
};

const STORAGE_KEY = "minijinja-playground:state";
const HASH_PREFIX = "#code/";

function isState(value: unknown): value is PlaygroundState {
  const state = value as PlaygroundState;
  return (
    typeof state === "object" &&
    state !== null &&
    Array.isArray(state.files) &&
    state.files.length > 0 &&
    state.files.every(
      (f) => typeof f.name === "string" && typeof f.source === "string",
    ) &&
    typeof state.entry === "string" &&
    typeof state.context === "string"
  );
}

function normalize(state: PlaygroundState): PlaygroundState {
  const settings = { ...DEFAULT_SETTINGS, ...state.settings };
  const entry = state.files.some((f) => f.name === state.entry)
    ? state.entry
    : state.files[0].name;
  return { ...state, entry, settings };
}

function toBase64Url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

function fromBase64Url(text: string): Uint8Array {
  const binary = atob(text.replace(/-/g, "+").replace(/_/g, "/"));
  return Uint8Array.from(binary, (c) => c.charCodeAt(0));
}

async function transform(
  bytes: Uint8Array,
  stream: CompressionStream | DecompressionStream,
): Promise<Uint8Array> {
  const response = new Response(
    new Blob([bytes as BlobPart]).stream().pipeThrough(stream),
  );
  return new Uint8Array(await response.arrayBuffer());
}

/** Encodes the state into a URL hash. */
export async function encodeHash(state: PlaygroundState): Promise<string> {
  const json = new TextEncoder().encode(JSON.stringify(state));
  const compressed = await transform(
    json,
    new CompressionStream("deflate-raw"),
  );
  return HASH_PREFIX + toBase64Url(compressed);
}

/** Decodes the state from a URL hash. */
export async function decodeHash(
  hash: string,
): Promise<PlaygroundState | null> {
  if (!hash.startsWith(HASH_PREFIX)) {
    return null;
  }
  try {
    const compressed = fromBase64Url(hash.slice(HASH_PREFIX.length));
    const json = await transform(
      compressed,
      new DecompressionStream("deflate-raw"),
    );
    const state: unknown = JSON.parse(new TextDecoder().decode(json));
    return isState(state) ? normalize(state) : null;
  } catch {
    return null;
  }
}

export function loadStoredState(): PlaygroundState | null {
  try {
    const value: unknown = JSON.parse(
      localStorage.getItem(STORAGE_KEY) ?? "null",
    );
    return isState(value) ? normalize(value) : null;
  } catch {
    return null;
  }
}

export function storeState(state: PlaygroundState) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // storage might be full or disabled
  }
}

/** Reads a UI preference from local storage. */
export function getPreference<T>(key: string, defaultValue: T): T {
  try {
    const value = localStorage.getItem(`minijinja-playground:${key}`);
    return value === null ? defaultValue : (JSON.parse(value) as T);
  } catch {
    return defaultValue;
  }
}

/** Writes a UI preference to local storage. */
export function setPreference<T>(key: string, value: T) {
  try {
    localStorage.setItem(`minijinja-playground:${key}`, JSON.stringify(value));
  } catch {
    // ignore
  }
}
