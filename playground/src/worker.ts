// Renders templates off the main thread so that runaway templates do not
// block the UI.  The main thread terminates this worker on timeouts.
import {
  Environment,
  SafeString,
  TemplateError,
  markSafe,
  passState,
} from "minijinja-js";
import { addDateTimeSupport } from "minijinja-js/datetime";
import type { ErrorInfo, RenderRequest, RenderResult } from "./protocol";

const CONFIG_URL = "config.js";
const CONFIG_PARAMS = [
  "env",
  "passState",
  "markSafe",
  "SafeString",
  "TemplateError",
];

type ConfigFunction = (env: Environment, ...helpers: unknown[]) => void;

// The function constructor adds a header before the body which offsets the
// line numbers in stack traces.  This determines that offset.
const LINE_OFFSET = (() => {
  try {
    new Function(
      ...CONFIG_PARAMS,
      "throw new Error()\n//# sourceURL=probe.js",
    )();
  } catch (err) {
    const match = /probe\.js:(\d+)/.exec(String((err as Error).stack));
    if (match) {
      return Number(match[1]) - 1;
    }
  }
  return 0;
})();

/** Finds the line in the config code from the stack of an error (or its causes). */
function configLine(err: unknown): number | undefined {
  for (let current = err; current instanceof Error; current = current.cause) {
    const match = /config\.js:(\d+)/.exec(String(current.stack));
    if (match) {
      return Number(match[1]) - LINE_OFFSET;
    }
  }
  return undefined;
}

function toErrorInfo(err: unknown): ErrorInfo {
  if (err instanceof TemplateError) {
    return {
      message: err.message,
      kind: err.kind,
      templateName: err.templateName,
      line: err.line,
      range: err.range,
      configLine: configLine(err.cause),
    };
  }
  return {
    message: String(err instanceof Error ? err.message : err),
    configLine: configLine(err),
  };
}

let compiled: { source: string; func: ConfigFunction } | null = null;

function compileConfig(source: string): ConfigFunction {
  if (compiled?.source !== source) {
    const func = new Function(
      ...CONFIG_PARAMS,
      `${source}\n//# sourceURL=${CONFIG_URL}`,
    ) as ConfigFunction;
    compiled = { source, func };
  }
  return compiled.func;
}

function runConfig(env: Environment, source: string) {
  compileConfig(source)(env, passState, markSafe, SafeString, TemplateError);
}

function createEnvironment({ settings }: RenderRequest["state"]): Environment {
  const env = new Environment();
  env.debug = true;
  env.pycompat = settings.pycompat;
  env.trimBlocks = settings.trimBlocks;
  env.lstripBlocks = settings.lstripBlocks;
  env.keepTrailingNewline = settings.keepTrailingNewline;
  env.undefinedBehavior = settings.undefinedBehavior;
  env.syntax = settings.syntax;
  addDateTimeSupport(env);
  return env;
}

function handle(request: RenderRequest): RenderResult {
  const { state } = request;
  const result: RenderResult = { id: request.id, diagnostics: [] };

  let env: Environment;
  try {
    env = createEnvironment(state);
  } catch (err) {
    result.error = toErrorInfo(err);
    return result;
  }

  if (request.runConfig && state.config.trim() !== "") {
    try {
      runConfig(env, state.config);
    } catch (err) {
      result.configError = toErrorInfo(err);
      result.error = result.configError;
      env.free();
      return result;
    }
  }

  try {
    // Templates are provided through a loader (rather than added upfront)
    // so that templates depending on a broken template report its error
    // instead of the template not being found.
    const sources = new Map(state.files.map((f) => [f.name, f.source]));
    env.setLoader((name) => sources.get(name));
    for (const file of state.files) {
      try {
        // compiles (and caches) the template to report syntax errors
        env.undeclaredVariablesInTemplate(file.name);
      } catch (err) {
        result.diagnostics.push(toErrorInfo(err));
      }
    }

    let context: unknown = {};
    try {
      context = state.context.trim() === "" ? {} : JSON.parse(state.context);
    } catch (err) {
      result.contextError = String(err instanceof Error ? err.message : err);
    }

    const entryError = result.diagnostics.find(
      (d) => d.templateName === state.entry,
    );
    if (entryError) {
      result.error = entryError;
    } else if (result.contextError === undefined) {
      try {
        const start = performance.now();
        result.output = env.renderTemplate(state.entry, context as object);
        result.renderTime = performance.now() - start;
      } catch (err) {
        result.error = toErrorInfo(err);
        result.diagnostics.push(result.error);
      }
    }

    try {
      result.variables = env.undeclaredVariablesInTemplate(state.entry, true);
    } catch {
      // errors are already reported
    }

    const file = state.files.find((f) => f.name === request.inspectFile);
    if (file && request.inspect) {
      try {
        if (request.inspect === "tokens") {
          result.tokens = env.unstableTokenize(file.source);
        } else if (request.inspect === "ast") {
          result.ast = env.unstableParse(file.source, file.name);
        } else {
          result.instructions = env.unstableCompile(file.source, file.name);
        }
      } catch (err) {
        result.inspectError = toErrorInfo(err);
      }
    }
  } finally {
    env.free();
  }

  return result;
}

self.onmessage = (event: MessageEvent<RenderRequest>) => {
  let result: RenderResult;
  try {
    result = handle(event.data);
  } catch (err) {
    result = {
      id: event.data.id,
      diagnostics: [],
      error: toErrorInfo(err),
    };
  }
  self.postMessage(result);
};

self.postMessage({ ready: true });
