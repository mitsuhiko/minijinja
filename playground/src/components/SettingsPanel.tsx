import type { SyntaxConfig, UndefinedBehavior } from "minijinja-js";
import { DEFAULT_SETTINGS, DEFAULT_SYNTAX, type Settings } from "../state";

interface Props {
  settings: Settings;
  onChange: (settings: Settings) => void;
}

const FLAGS: {
  key: "pycompat" | "trimBlocks" | "lstripBlocks" | "keepTrailingNewline";
  title: string;
  help: string;
}[] = [
  {
    key: "pycompat",
    title: "Python compatibility",
    help: "Enables Python methods like dict.items()",
  },
  {
    key: "trimBlocks",
    title: "Trim blocks",
    help: "Removes the first newline after a block tag",
  },
  {
    key: "lstripBlocks",
    title: "Lstrip blocks",
    help: "Strips whitespace before a block tag",
  },
  {
    key: "keepTrailingNewline",
    title: "Keep trailing newline",
    help: "Keeps the final newline of a template",
  },
];

const UNDEFINED_BEHAVIORS: { value: UndefinedBehavior; title: string }[] = [
  { value: "lenient", title: "Lenient" },
  { value: "chainable", title: "Chainable" },
  { value: "semi_strict", title: "Semi-strict" },
  { value: "strict", title: "Strict" },
];

const SYNTAX_FIELDS: [keyof SyntaxConfig, keyof SyntaxConfig, string][] = [
  ["blockStart", "blockEnd", "Blocks"],
  ["variableStart", "variableEnd", "Variables"],
  ["commentStart", "commentEnd", "Comments"],
];

export function SettingsPanel({ settings, onChange }: Props) {
  const setSyntax = (key: keyof SyntaxConfig, value: string) => {
    const syntax = { ...settings.syntax };
    if (value === "") {
      delete syntax[key];
    } else {
      (syntax as Record<string, string>)[key] = value;
    }
    onChange({ ...settings, syntax });
  };

  const syntaxInput = (key: keyof SyntaxConfig, label: string) => (
    <input
      type="text"
      aria-label={label}
      value={settings.syntax[key] ?? ""}
      placeholder={DEFAULT_SYNTAX[key] ?? "off"}
      spellCheck={false}
      onChange={(event) => setSyntax(key, event.target.value)}
    />
  );

  return (
    <div className="settings">
      <section>
        <h3>Behavior</h3>
        {FLAGS.map((flag) => (
          <label key={flag.key} className="setting-flag" title={flag.help}>
            <input
              type="checkbox"
              checked={settings[flag.key]}
              onChange={(event) =>
                onChange({ ...settings, [flag.key]: event.target.checked })
              }
            />
            <span>
              {flag.title}
              <small>{flag.help}</small>
            </span>
          </label>
        ))}
        <label className="setting-row">
          <span>Undefined behavior</span>
          <select
            value={settings.undefinedBehavior}
            onChange={(event) =>
              onChange({
                ...settings,
                undefinedBehavior: event.target.value as UndefinedBehavior,
              })
            }
          >
            {UNDEFINED_BEHAVIORS.map((b) => (
              <option key={b.value} value={b.value}>
                {b.title}
              </option>
            ))}
          </select>
        </label>
      </section>
      <section>
        <h3>Syntax</h3>
        <div className="syntax-grid">
          {SYNTAX_FIELDS.map(([start, end, label]) => (
            <div key={label} className="syntax-row">
              <span>{label}</span>
              {syntaxInput(start, `${label} start`)}
              {syntaxInput(end, `${label} end`)}
            </div>
          ))}
          <div className="syntax-row">
            <span>Line statements</span>
            {syntaxInput("lineStatementPrefix", "Line statement prefix")}
          </div>
          <div className="syntax-row">
            <span>Line comments</span>
            {syntaxInput("lineCommentPrefix", "Line comment prefix")}
          </div>
        </div>
      </section>
      <button
        className="button subtle"
        onClick={() => onChange(DEFAULT_SETTINGS)}
      >
        Reset to defaults
      </button>
    </div>
  );
}
