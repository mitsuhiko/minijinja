import { useEffect, useRef, useState } from "react";
import type { TemplateFile } from "../state";

interface Props {
  files: TemplateFile[];
  active: string;
  entry: string;
  errors: Set<string>;
  onSelect: (name: string) => void;
  onAdd: () => string;
  onRename: (from: string, to: string) => void;
  onRemove: (name: string) => void;
  onSetEntry: (name: string) => void;
}

function RenameInput({
  name,
  existing,
  onDone,
}: {
  name: string;
  existing: string[];
  onDone: (name: string | null) => void;
}) {
  const [value, setValue] = useState(name);
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const input = ref.current!;
    input.focus();
    const dot = name.lastIndexOf(".");
    input.setSelectionRange(0, dot > 0 ? dot : name.length);
  }, [name]);

  const trimmed = value.trim();
  const invalid =
    trimmed === "" || (trimmed !== name && existing.includes(trimmed));

  return (
    <input
      ref={ref}
      className={`tab-rename${invalid ? " invalid" : ""}`}
      value={value}
      size={Math.max(value.length, 4)}
      aria-label="File name"
      onChange={(event) => setValue(event.target.value)}
      onBlur={() => onDone(invalid ? null : trimmed)}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          onDone(invalid ? null : trimmed);
        } else if (event.key === "Escape") {
          onDone(null);
        }
      }}
    />
  );
}

export function FileTabs({
  files,
  active,
  entry,
  errors,
  onSelect,
  onAdd,
  onRename,
  onRemove,
  onSetEntry,
}: Props) {
  const [renaming, setRenaming] = useState<string | null>(null);
  const names = files.map((f) => f.name);

  return (
    <div className="tabs" role="tablist" aria-label="Templates">
      {files.map((file) => (
        <div
          key={file.name}
          role="tab"
          aria-selected={file.name === active}
          className={`tab${file.name === active ? " active" : ""}`}
          onClick={() => onSelect(file.name)}
          onDoubleClick={() => setRenaming(file.name)}
          title="Double click to rename"
        >
          {renaming === file.name ? (
            <RenameInput
              name={file.name}
              existing={names}
              onDone={(name) => {
                setRenaming(null);
                if (name && name !== file.name) {
                  onRename(file.name, name);
                }
              }}
            />
          ) : (
            <span className="tab-name">{file.name}</span>
          )}
          {errors.has(file.name) && (
            <span className="tab-error" title="This template has an error" />
          )}
          {files.length > 1 && (
            <button
              className={`tab-entry${file.name === entry ? " is-entry" : ""}`}
              title={
                file.name === entry
                  ? "This template is rendered"
                  : "Render this template"
              }
              aria-label={`Render ${file.name}`}
              onClick={(event) => {
                event.stopPropagation();
                onSetEntry(file.name);
              }}
            >
              ▶
            </button>
          )}
          {files.length > 1 && (
            <button
              className="tab-close"
              title="Remove template"
              aria-label={`Remove ${file.name}`}
              onClick={(event) => {
                event.stopPropagation();
                if (
                  file.source.trim() === "" ||
                  confirm(`Remove the template "${file.name}"?`)
                ) {
                  onRemove(file.name);
                }
              }}
            >
              ×
            </button>
          )}
        </div>
      ))}
      <button
        className="tab-add"
        title="Add template"
        aria-label="Add template"
        onClick={() => setRenaming(onAdd())}
      >
        +
      </button>
    </div>
  );
}
