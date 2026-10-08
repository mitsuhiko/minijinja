import type { PlaygroundState } from "./state";

export type InspectView = "tokens" | "ast" | "instructions";

export interface RenderRequest {
  id: number;
  state: PlaygroundState;
  /** Whether the config code may run (it might come from a shared link). */
  runConfig: boolean;
  /** The file to inspect for machinery views. */
  inspectFile: string;
  inspect: InspectView | null;
}

export interface ErrorInfo {
  message: string;
  kind?: string;
  templateName?: string;
  line?: number;
  /** Byte range in the template source. */
  range?: { start: number; end: number };
  /** The line in the config code that caused the error, if any. */
  configLine?: number;
}

export interface Span {
  start_line: number;
  start_col: number;
  start_offset: number;
  end_line: number;
  end_col: number;
  end_offset: number;
}

export interface Token {
  name: string;
  payload?: unknown;
}

export interface Instruction {
  op: string;
  arg?: unknown;
}

export type LogLevel = "log" | "info" | "warn" | "error" | "debug";

export interface LogEntry {
  level: LogLevel;
  message: string;
  /** Whether the message was logged by the config code or during the render. */
  phase: "config" | "render";
}

export interface RenderResult {
  id: number;
  output?: string;
  error?: ErrorInfo;
  contextError?: string;
  /** Set if running the config code failed. */
  configError?: ErrorInfo;
  /** Errors of all templates (syntax errors and the render error). */
  diagnostics: ErrorInfo[];
  renderTime?: number;
  /** Console messages logged by the config code and callbacks. */
  logs?: LogEntry[];
  /** The number of messages that were dropped because there were too many. */
  droppedLogs?: number;
  variables?: string[];
  tokens?: [Token, Span][];
  ast?: unknown;
  instructions?: Record<string, Instruction[]>;
  inspectError?: ErrorInfo;
}
