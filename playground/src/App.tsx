import { json, jsonParseLinter } from "@codemirror/lang-json";
import type { Diagnostic } from "@codemirror/lint";
import { linter } from "@codemirror/lint";
import packageInfo from "minijinja-js/package.json";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { CodeEditor, type Selection } from "./components/CodeEditor";
import { FileTabs } from "./components/FileTabs";
import {
  OUTPUT_VIEWS,
  OutputPanel,
  type OutputView,
} from "./components/OutputPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { Splitter } from "./components/Splitter";
import { DEFAULT_STATE, EXAMPLES } from "./examples";
import { templateLanguage } from "./languages";
import { jinjaHighlight } from "./theme";
import type { ErrorInfo, InspectView, RenderResult } from "./protocol";
import { Renderer } from "./renderer";
import {
  decodeHash,
  encodeHash,
  getPreference,
  loadStoredState,
  type PlaygroundState,
  setPreference,
  storeState,
} from "./state";
import { autoEscapeFor, byteToIndex, extension } from "./utils";

type Theme = "system" | "light" | "dark";

const CONTEXT_EXTENSIONS = [json(), linter(jsonParseLinter())];
const COMMIT = import.meta.env.VITE_COMMIT as string | undefined;

function usePreference<T>(key: string, defaultValue: T) {
  const [value, setValue] = useState<T>(() => getPreference(key, defaultValue));
  const update = useCallback(
    (newValue: T) => {
      setValue(newValue);
      setPreference(key, newValue);
    },
    [key],
  );
  return [value, update] as const;
}

function useDarkMode(theme: Theme): boolean {
  const query = useMemo(
    () => window.matchMedia("(prefers-color-scheme: dark)"),
    [],
  );
  const [systemDark, setSystemDark] = useState(query.matches);
  useEffect(() => {
    const listener = () => setSystemDark(query.matches);
    query.addEventListener("change", listener);
    return () => query.removeEventListener("change", listener);
  }, [query]);
  const dark = theme === "system" ? systemDark : theme === "dark";
  useEffect(() => {
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  }, [dark]);
  return dark;
}

function toDiagnostic(error: ErrorInfo, source: string): Diagnostic {
  let from = 0;
  let to = 0;
  if (error.range) {
    from = byteToIndex(source, error.range.start);
    to = byteToIndex(source, error.range.end);
  } else if (error.line) {
    const lines = source.split("\n");
    from = lines.slice(0, error.line - 1).reduce((n, l) => n + l.length + 1, 0);
    to = from + (lines[error.line - 1]?.length ?? 0);
  }
  return {
    from,
    to: Math.max(to, from),
    severity: "error",
    message: error.message.split("\n")[0],
  };
}

function uniqueName(names: string[], base: string, ext: string): string {
  let name = `${base}.${ext}`;
  for (let i = 2; names.includes(name); i++) {
    name = `${base}-${i}.${ext}`;
  }
  return name;
}

async function initialState(): Promise<PlaygroundState> {
  return (
    (await decodeHash(location.hash)) ?? loadStoredState() ?? DEFAULT_STATE
  );
}

export function App() {
  const [state, setState] = useState<PlaygroundState | null>(null);
  useEffect(() => {
    void initialState().then(setState);
    const onHashChange = () => {
      void decodeHash(location.hash).then((s) => s && setState(s));
    };
    window.addEventListener("hashchange", onHashChange);
    return () => window.removeEventListener("hashchange", onHashChange);
  }, []);
  return state ? <Playground state={state} setState={setState} /> : null;
}

function Playground({
  state,
  setState,
}: {
  state: PlaygroundState;
  setState: React.Dispatch<React.SetStateAction<PlaygroundState | null>>;
}) {
  const update = useCallback(
    (fn: (s: PlaygroundState) => PlaygroundState) =>
      setState((s) => (s ? fn(s) : s)),
    [setState],
  );

  const [activeFile, setActiveFile] = useState(state.entry);
  const [result, setResult] = useState<RenderResult | null>(null);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [view, setView] = usePreference<OutputView>("view", "output");
  const [showWhitespace, setShowWhitespace] = usePreference(
    "whitespace",
    false,
  );
  const [splitX, setSplitX] = usePreference("split-x", 0.5);
  const [splitY, setSplitY] = usePreference("split-y", 0.38);
  const [theme, setTheme] = usePreference<Theme>("theme", "system");
  const [copied, setCopied] = useState(false);
  const dark = useDarkMode(theme);
  const baseline = useRef(JSON.stringify(state));

  // the active file might disappear when a new state is loaded
  const file = state.files.find((f) => f.name === activeFile) ?? state.files[0];

  // persist the state and keep the URL shareable
  useEffect(() => {
    const timer = setTimeout(() => {
      storeState(state);
      void encodeHash(state).then((hash) => {
        history.replaceState(null, "", hash);
      });
    }, 250);
    return () => clearTimeout(timer);
  }, [state]);

  // render in the worker
  const renderer = useRef<Renderer | null>(null);
  useEffect(() => {
    renderer.current = new Renderer(setResult);
    return () => renderer.current?.dispose();
  }, []);
  const inspect = OUTPUT_VIEWS.find((v) => v.id === view)?.inspect
    ? (view as InspectView)
    : null;
  useEffect(() => {
    renderer.current?.render({ state, inspectFile: file.name, inspect });
  }, [state, file.name, inspect]);

  // the parsed context for completions and the variables view
  const context = useMemo(() => {
    try {
      return JSON.parse(state.context) as unknown;
    } catch {
      return null;
    }
  }, [state.context]);
  const contextRef = useRef(context);
  contextRef.current = context;
  const contextKeys =
    typeof context === "object" && context !== null
      ? Object.keys(context).join("\0")
      : "";
  const language = useMemo(
    () =>
      templateLanguage(file.name, contextRef.current, () => contextRef.current),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [extension(file.name), contextKeys],
  );

  const diagnostics = useMemo(() => {
    const seen = new Set<string>();
    return (result?.diagnostics ?? [])
      .filter((d) => d.templateName === file.name)
      .map((d) => toDiagnostic(d, file.source))
      .filter((d) => {
        const key = `${d.from}:${d.to}:${d.message}`;
        return seen.has(key) ? false : (seen.add(key), true);
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [result, file.name]);
  const errorFiles = useMemo(
    () => new Set((result?.diagnostics ?? []).map((d) => d.templateName ?? "")),
    [result],
  );

  const loadExample = (id: string) => {
    const example = EXAMPLES.find((e) => e.id === id);
    if (!example) {
      return;
    }
    if (
      JSON.stringify(state) !== baseline.current &&
      !confirm("Replace your templates with the example?")
    ) {
      return;
    }
    baseline.current = JSON.stringify(example.state);
    setState(example.state);
    setActiveFile(example.state.entry);
  };

  const share = async () => {
    const url = new URL(location.href);
    url.hash = await encodeHash(state);
    await navigator.clipboard.writeText(url.toString());
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const formatContext = () => {
    if (context !== null) {
      update((s) => ({
        ...s,
        context: JSON.stringify(context, null, 2) + "\n",
      }));
    }
  };

  return (
    <div className="app">
      <header className="header">
        <a className="brand" href="https://github.com/mitsuhiko/minijinja">
          <img src="logo.png" alt="" width={28} height={28} />
          <span>
            MiniJinja <span className="brand-sub">Playground</span>
          </span>
        </a>
        <select
          className="examples"
          value=""
          aria-label="Load an example"
          onChange={(event) => loadExample(event.target.value)}
        >
          <option value="" disabled>
            Examples…
          </option>
          {EXAMPLES.map((example) => (
            <option key={example.id} value={example.id}>
              {example.title}
            </option>
          ))}
        </select>
        <div className="header-spacer" />
        <button className="button" popoverTarget="settings-popover">
          Settings
        </button>
        <button className="button primary" onClick={() => void share()}>
          {copied ? "Link copied!" : "Share"}
        </button>
        <button
          className="button icon"
          title={`Theme: ${theme}`}
          aria-label={`Theme: ${theme}`}
          onClick={() =>
            setTheme(
              theme === "system"
                ? "light"
                : theme === "light"
                  ? "dark"
                  : "system",
            )
          }
        >
          {theme === "system" ? "◐" : theme === "light" ? "☀" : "☾"}
        </button>
        <nav className="links">
          <a href="https://docs.rs/minijinja/">Docs</a>
          <a href="https://github.com/mitsuhiko/minijinja/tree/main/minijinja-js">
            minijinja-js {packageInfo.version}
          </a>
          {COMMIT && (
            <a href={`https://github.com/mitsuhiko/minijinja/commit/${COMMIT}`}>
              {COMMIT.slice(0, 7)}
            </a>
          )}
        </nav>
        <div id="settings-popover" className="popover" popover="auto">
          <SettingsPanel
            settings={state.settings}
            onChange={(settings) => update((s) => ({ ...s, settings }))}
          />
        </div>
      </header>

      <main
        className="workspace"
        style={
          { "--split-x": splitX, "--split-y": splitY } as React.CSSProperties
        }
      >
        <div className="pane template-pane">
          <div className="pane-header">
            <FileTabs
              files={state.files}
              active={file.name}
              entry={state.entry}
              errors={errorFiles}
              onSelect={setActiveFile}
              onAdd={() => {
                const name = uniqueName(
                  state.files.map((f) => f.name),
                  "untitled",
                  extension(file.name) || "html",
                );
                update((s) => ({
                  ...s,
                  files: [...s.files, { name, source: "" }],
                }));
                setActiveFile(name);
                return name;
              }}
              onRename={(from, to) => {
                update((s) => ({
                  ...s,
                  files: s.files.map((f) =>
                    f.name === from ? { ...f, name: to } : f,
                  ),
                  entry: s.entry === from ? to : s.entry,
                }));
                if (activeFile === from) {
                  setActiveFile(to);
                }
              }}
              onRemove={(name) => {
                update((s) => {
                  const files = s.files.filter((f) => f.name !== name);
                  return {
                    ...s,
                    files,
                    entry: s.entry === name ? files[0].name : s.entry,
                  };
                });
                if (file.name === name) {
                  setActiveFile(state.files.find((f) => f.name !== name)!.name);
                }
              }}
              onSetEntry={(name) => update((s) => ({ ...s, entry: name }))}
            />
            <div className="pane-tools">
              <span
                className="badge"
                title="Auto escaping is determined by the file extension"
              >
                escape: {autoEscapeFor(file.name)}
              </span>
            </div>
          </div>
          <div className="pane-body">
            <CodeEditor
              docKey={`template:${file.name}`}
              label={`Template ${file.name}`}
              value={file.source}
              onChange={(source) =>
                update((s) => ({
                  ...s,
                  files: s.files.map((f) =>
                    f.name === file.name ? { ...f, source } : f,
                  ),
                }))
              }
              language={language}
              extensions={jinjaHighlight}
              dark={dark}
              diagnostics={diagnostics}
              selection={selection}
            />
          </div>
        </div>

        <Splitter direction="horizontal" onResize={setSplitX} />

        <div className="side">
          <div className="pane context-pane">
            <div className="pane-header">
              <span className="pane-title">Context</span>
              <div className="pane-tools">
                <button
                  className="button subtle small"
                  onClick={formatContext}
                  disabled={context === null}
                >
                  Format
                </button>
              </div>
            </div>
            <div className="pane-body">
              <CodeEditor
                docKey="context"
                label="Context (JSON)"
                value={state.context}
                onChange={(value) => update((s) => ({ ...s, context: value }))}
                language={CONTEXT_EXTENSIONS}
                dark={dark}
              />
            </div>
          </div>

          <Splitter direction="vertical" onResize={setSplitY} />

          <OutputPanel
            result={result}
            view={view}
            onViewChange={setView}
            entry={state.entry}
            inspectFile={file.name}
            context={context}
            showWhitespace={showWhitespace}
            onShowWhitespaceChange={setShowWhitespace}
            onSelectSpan={(name, start, end) => {
              const target = state.files.find((f) => f.name === name);
              if (target) {
                setActiveFile(name);
                setSelection({
                  from: byteToIndex(target.source, start),
                  to: byteToIndex(target.source, end),
                  nonce: Math.random(),
                });
              }
            }}
          />
        </div>
      </main>
    </div>
  );
}
