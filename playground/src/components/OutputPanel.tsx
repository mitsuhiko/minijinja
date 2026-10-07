import { type ReactNode, useMemo } from "react";
import type { ErrorInfo, InspectView, RenderResult, Span } from "../protocol";

export type OutputView = "output" | "preview" | "variables" | InspectView;

export const OUTPUT_VIEWS: {
  id: OutputView;
  title: string;
  inspect?: boolean;
}[] = [
  { id: "output", title: "Output" },
  { id: "preview", title: "Preview" },
  { id: "variables", title: "Variables" },
  { id: "tokens", title: "Tokens", inspect: true },
  { id: "ast", title: "AST", inspect: true },
  { id: "instructions", title: "Instructions", inspect: true },
];

interface Props {
  result: RenderResult | null;
  view: OutputView;
  onViewChange: (view: OutputView) => void;
  entry: string;
  inspectFile: string;
  context: unknown;
  showWhitespace: boolean;
  onShowWhitespaceChange: (yes: boolean) => void;
  onSelectSpan: (file: string, start: number, end: number) => void;
}

function ErrorBox({
  error,
  title,
}: {
  error: ErrorInfo | string;
  title?: string;
}) {
  const message = typeof error === "string" ? error : error.message;
  return (
    <div className="error-box" role="alert">
      {title && <div className="error-title">{title}</div>}
      <pre>{message}</pre>
    </div>
  );
}

function WhitespaceText({ text }: { text: string }) {
  const parts: ReactNode[] = [];
  let buffer = "";
  const flush = () => {
    if (buffer) {
      parts.push(buffer);
      buffer = "";
    }
  };
  for (const char of text) {
    if (char === " " || char === "\t" || char === "\n") {
      flush();
      parts.push(
        <span key={parts.length} className="ws">
          {char === " " ? "·" : char === "\t" ? "→   " : "↵"}
        </span>,
      );
      if (char === "\n") {
        parts.push("\n");
      }
    } else {
      buffer += char;
    }
  }
  flush();
  return <>{parts}</>;
}

function TextOutput({
  output,
  showWhitespace,
}: {
  output: string;
  showWhitespace: boolean;
}) {
  if (output === "") {
    return <div className="empty">The template rendered an empty string.</div>;
  }
  return (
    <pre className="text-output">
      {showWhitespace ? <WhitespaceText text={output} /> : output}
    </pre>
  );
}

function hasPath(context: unknown, path: string): boolean {
  let value = context;
  for (const segment of path.split(".")) {
    if (typeof value !== "object" || value === null || !(segment in value)) {
      return false;
    }
    value = (value as Record<string, unknown>)[segment];
  }
  return true;
}

function VariablesView({
  variables,
  context,
}: {
  variables: string[];
  context: unknown;
}) {
  if (variables.length === 0) {
    return (
      <div className="empty">
        The template does not reference any undeclared variables.
      </div>
    );
  }
  return (
    <div className="inspect">
      <p className="hint">
        Variables the template references but does not define itself. These are
        resolved from the context or the globals.
      </p>
      <table className="data-table">
        <tbody>
          {variables.map((name) => {
            const found = hasPath(context, name);
            return (
              <tr key={name}>
                <td>
                  <code>{name}</code>
                </td>
                <td className={found ? "ok" : "muted"}>
                  {found ? "in context" : "not in context"}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function formatSpan(span: Span): string {
  return `${span.start_line}:${span.start_col}–${span.end_line}:${span.end_col}`;
}

function isSpan(value: unknown): value is Span {
  return (
    typeof value === "object" &&
    value !== null &&
    "start_offset" in value &&
    "end_offset" in value
  );
}

function SpanLink({
  span,
  onSelect,
}: {
  span: Span;
  onSelect: (span: Span) => void;
}) {
  return (
    <button
      className="span-link"
      onClick={() => onSelect(span)}
      title="Select in editor"
    >
      {formatSpan(span)}
    </button>
  );
}

function TokensView({
  tokens,
  onSelect,
}: {
  tokens: NonNullable<RenderResult["tokens"]>;
  onSelect: (span: Span) => void;
}) {
  return (
    <table className="data-table">
      <tbody>
        {tokens.map(([token, span], idx) => (
          <tr key={idx}>
            <td>
              <code className="strong">{token.name}</code>
            </td>
            <td className="payload">
              {token.payload !== undefined && (
                <code>{JSON.stringify(token.payload)}</code>
              )}
            </td>
            <td className="right">
              <SpanLink span={span} onSelect={onSelect} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function nodeLabel(value: unknown): string | null {
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    for (const key of ["stmt", "expr"]) {
      const label = (value as Record<string, unknown>)[key];
      if (typeof label === "string") {
        return label;
      }
    }
  }
  return null;
}

function AstNode({
  name,
  value,
  depth,
  onSelect,
}: {
  name?: string;
  value: unknown;
  depth: number;
  onSelect: (span: Span) => void;
}) {
  const key = name !== undefined && <span className="ast-key">{name}: </span>;

  if (typeof value !== "object" || value === null) {
    return (
      <div className="ast-leaf">
        {key}
        <code className="ast-value">{JSON.stringify(value)}</code>
      </div>
    );
  }

  if (isSpan(value)) {
    return (
      <div className="ast-leaf">
        {key}
        <SpanLink span={value} onSelect={onSelect} />
      </div>
    );
  }

  // spanned nodes are serialized as [node, span]
  if (Array.isArray(value) && value.length === 2 && isSpan(value[1])) {
    return (
      <AstNode name={name} value={value[0]} depth={depth} onSelect={onSelect} />
    );
  }

  const label = nodeLabel(value);
  const entries = Array.isArray(value)
    ? value.map((item, idx) => [String(idx), item] as const)
    : Object.entries(value).filter(([k]) => k !== "stmt" && k !== "expr");

  // inline the content of nodes
  if (label && entries.length === 1 && entries[0][0] === "inner") {
    const inner = entries[0][1];
    const span =
      Array.isArray(inner) && inner.length === 2 && isSpan(inner[1])
        ? inner[1]
        : null;
    const body = span ? (inner as unknown[])[0] : inner;
    return (
      <details className="ast-node" open={depth < 12}>
        <summary>
          {key}
          <span className="ast-label">{label}</span>
          {span && <SpanLink span={span} onSelect={onSelect} />}
        </summary>
        <div className="ast-children">
          {typeof body === "object" && body !== null && !Array.isArray(body) ? (
            Object.entries(body).map(([k, v]) => (
              <AstNode
                key={k}
                name={k}
                value={v}
                depth={depth + 1}
                onSelect={onSelect}
              />
            ))
          ) : (
            <AstNode value={body} depth={depth + 1} onSelect={onSelect} />
          )}
        </div>
      </details>
    );
  }

  if (entries.length === 0) {
    return (
      <div className="ast-leaf">
        {key}
        <code className="ast-value">{Array.isArray(value) ? "[]" : "{}"}</code>
      </div>
    );
  }

  return (
    <details className="ast-node" open={depth < 12}>
      <summary>
        {key}
        {label ? (
          <span className="ast-label">{label}</span>
        ) : (
          <span className="muted">
            {Array.isArray(value) ? `[${entries.length}]` : "{…}"}
          </span>
        )}
      </summary>
      <div className="ast-children">
        {entries.map(([k, v]) => (
          <AstNode
            key={k}
            name={k}
            value={v}
            depth={depth + 1}
            onSelect={onSelect}
          />
        ))}
      </div>
    </details>
  );
}

function InstructionsView({
  instructions,
}: {
  instructions: NonNullable<RenderResult["instructions"]>;
}) {
  return (
    <>
      {Object.entries(instructions).map(([block, instrs]) => (
        <section key={block} className="instructions-block">
          <h3>{block === "<root>" ? "Root" : `Block "${block}"`}</h3>
          <table className="data-table">
            <tbody>
              {instrs.map((instr, idx) => (
                <tr key={idx}>
                  <td className="muted right index">{idx}</td>
                  <td>
                    <code className="strong">{instr.op}</code>
                  </td>
                  <td className="payload">
                    {instr.arg !== undefined && (
                      <code>{JSON.stringify(instr.arg)}</code>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      ))}
    </>
  );
}

export function OutputPanel({
  result,
  view,
  onViewChange,
  entry,
  inspectFile,
  context,
  showWhitespace,
  onShowWhitespaceChange,
  onSelectSpan,
}: Props) {
  const isInspect = OUTPUT_VIEWS.find((v) => v.id === view)?.inspect ?? false;
  const selectSpan = (span: Span) =>
    onSelectSpan(inspectFile, span.start_offset, span.end_offset);

  const body = useMemo(() => {
    if (!result) {
      return <div className="empty">Loading…</div>;
    }
    if (isInspect) {
      if (result.inspectError) {
        return <ErrorBox error={result.inspectError} />;
      }
      if (view === "tokens" && result.tokens) {
        return <TokensView tokens={result.tokens} onSelect={selectSpan} />;
      }
      if (view === "ast" && result.ast) {
        return (
          <div className="ast">
            <AstNode value={result.ast} depth={0} onSelect={selectSpan} />
          </div>
        );
      }
      if (view === "instructions" && result.instructions) {
        return <InstructionsView instructions={result.instructions} />;
      }
      return <div className="empty">Loading…</div>;
    }
    if (view === "variables") {
      return (
        <VariablesView variables={result.variables ?? []} context={context} />
      );
    }
    if (result.contextError) {
      return (
        <ErrorBox
          title="The context is not valid JSON"
          error={result.contextError}
        />
      );
    }
    if (result.error) {
      return <ErrorBox error={result.error} />;
    }
    if (view === "preview") {
      return (
        <iframe
          className="preview"
          title="Rendered HTML preview"
          sandbox=""
          srcDoc={result.output ?? ""}
        />
      );
    }
    return (
      <TextOutput
        output={result.output ?? ""}
        showWhitespace={showWhitespace}
      />
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [result, view, context, showWhitespace, inspectFile]);

  return (
    <div className="pane output-pane">
      <div className="pane-header">
        <div className="tabs" role="tablist" aria-label="Output">
          {OUTPUT_VIEWS.map((v) => (
            <button
              key={v.id}
              role="tab"
              aria-selected={v.id === view}
              className={`tab${v.id === view ? " active" : ""}`}
              onClick={() => onViewChange(v.id)}
            >
              {v.title}
            </button>
          ))}
        </div>
        <div className="pane-tools">
          {view === "output" && (
            <label className="toggle" title="Show spaces, tabs and newlines">
              <input
                type="checkbox"
                checked={showWhitespace}
                onChange={(event) =>
                  onShowWhitespaceChange(event.target.checked)
                }
              />
              Whitespace
            </label>
          )}
          <span className="status">
            {isInspect ? (
              <>
                of <strong>{inspectFile}</strong>
              </>
            ) : (
              <>
                <strong>{entry}</strong>
                {result?.renderTime !== undefined && !result.error && (
                  <>
                    {" "}
                    ·{" "}
                    {result.renderTime < 1
                      ? "<1"
                      : result.renderTime.toFixed(1)}{" "}
                    ms
                  </>
                )}
              </>
            )}
          </span>
        </div>
      </div>
      <div className="pane-body output-body">{body}</div>
    </div>
  );
}
