// Captures console output in the render worker.
import { SafeString } from "minijinja-js";
import type { LogEntry, LogLevel } from "./protocol";

const MAX_ENTRIES = 500;
const MAX_MESSAGE_LENGTH = 10000;
const MAX_DEPTH = 4;
const LEVELS: LogLevel[] = ["log", "info", "warn", "error", "debug"];

let entries: LogEntry[] | null = null;
let dropped = 0;
let phase: LogEntry["phase"] = "render";

function formatKey(key: string): string {
  return /^[A-Za-z_$][\w$]*$/.test(key) ? key : JSON.stringify(key);
}

/** Formats a value similar to how browser consoles display them. */
export function inspect(
  value: unknown,
  depth = 0,
  seen = new Set<object>(),
): string {
  switch (typeof value) {
    case "string":
      return depth === 0 ? value : JSON.stringify(value);
    case "undefined":
      return "undefined";
    case "number":
    case "boolean":
      return String(value);
    case "bigint":
      return `${value}n`;
    case "symbol":
      return value.toString();
    case "function":
      return `[Function ${value.name || "(anonymous)"}]`;
  }
  if (value === null) {
    return "null";
  }
  const obj = value as object;
  if (obj instanceof SafeString) {
    return `SafeString(${JSON.stringify(String(obj))})`;
  }
  if (obj instanceof Error) {
    return `${obj.name}: ${obj.message}`;
  }
  if (obj instanceof Date) {
    return Number.isNaN(obj.getTime()) ? "Invalid Date" : obj.toISOString();
  }
  if (seen.has(obj)) {
    return "[Circular]";
  }
  if (depth >= MAX_DEPTH) {
    return Array.isArray(obj) ? "[Array]" : "[Object]";
  }
  seen.add(obj);
  try {
    const nested = (item: unknown) => inspect(item, depth + 1, seen);
    if (Array.isArray(obj)) {
      return `[${obj.map(nested).join(", ")}]`;
    }
    if (obj instanceof Map) {
      const items = [...obj].map(([k, v]) => `${nested(k)} => ${nested(v)}`);
      return `Map(${obj.size}) {${items.length ? ` ${items.join(", ")} ` : ""}}`;
    }
    if (obj instanceof Set) {
      const items = [...obj].map(nested);
      return `Set(${obj.size}) {${items.length ? ` ${items.join(", ")} ` : ""}}`;
    }
    if (ArrayBuffer.isView(obj) && !(obj instanceof DataView)) {
      const items = Array.from(obj as unknown as ArrayLike<number>, nested);
      return `${obj.constructor.name}(${items.length}) [${items.join(", ")}]`;
    }
    const name = obj.constructor?.name;
    if (isState(obj)) {
      return inspectState(obj, nested);
    }
    const items = Object.entries(obj)
      .filter(([key]) => !key.startsWith("__wbg"))
      .map(([key, item]) => `${formatKey(key)}: ${nested(item)}`);
    const body = items.length ? `{ ${items.join(", ")} }` : "{}";
    return name && name !== "Object" ? `${name} ${body}` : body;
  } finally {
    seen.delete(obj);
  }
}

/** Detects the engine state (the class name is not stable in minified builds). */
function isState(obj: object): obj is Record<string, unknown> {
  const state = obj as Record<string, unknown>;
  return (
    typeof state.lookup === "function" &&
    typeof state.applyFilter === "function" &&
    typeof state.performTest === "function"
  );
}

/** Formats the engine state (its properties are getters). */
function inspectState(
  state: Record<string, unknown>,
  nested: (value: unknown) => string,
): string {
  try {
    const items = [
      "name",
      "autoEscape",
      "undefinedBehavior",
      "currentBlock",
    ].map((key) => `${key}: ${nested(state[key])}`);
    return `State { ${items.join(", ")} }`;
  } catch {
    return "State (no longer valid)";
  }
}

function capture(level: LogLevel, args: unknown[]) {
  if (!entries) {
    return;
  }
  if (entries.length >= MAX_ENTRIES) {
    dropped++;
    return;
  }
  let message = args.map((arg) => inspect(arg)).join(" ");
  if (message.length > MAX_MESSAGE_LENGTH) {
    message = `${message.slice(0, MAX_MESSAGE_LENGTH)}… (truncated)`;
  }
  entries.push({ level, message, phase });
}

/** Replaces the console methods so that output can be captured. */
export function installConsoleCapture() {
  for (const level of LEVELS) {
    const original = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      capture(level, args);
      original(...args);
    };
  }
  const originalDir = console.dir.bind(console);
  console.dir = (value: unknown, ...rest: unknown[]) => {
    capture("log", [value]);
    originalDir(value, ...rest);
  };
}

/** Starts capturing console output. */
export function startCapture() {
  entries = [];
  dropped = 0;
  phase = "render";
}

/** Sets the phase that captured messages are attributed to. */
export function setPhase(value: LogEntry["phase"]) {
  phase = value;
}

/** Stops capturing and returns the captured messages. */
export function stopCapture(): { logs: LogEntry[]; dropped: number } {
  const rv = { logs: entries ?? [], dropped };
  entries = null;
  return rv;
}
