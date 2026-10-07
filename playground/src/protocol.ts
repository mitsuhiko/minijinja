import type { PlaygroundState } from "./state";

export type InspectView = "tokens" | "ast" | "instructions";

export interface RenderRequest {
  id: number;
  state: PlaygroundState;
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

export interface RenderResult {
  id: number;
  output?: string;
  error?: ErrorInfo;
  contextError?: string;
  /** Errors of all templates (syntax errors and the render error). */
  diagnostics: ErrorInfo[];
  renderTime?: number;
  variables?: string[];
  tokens?: [Token, Span][];
  ast?: unknown;
  instructions?: Record<string, Instruction[]>;
  inspectError?: ErrorInfo;
}
