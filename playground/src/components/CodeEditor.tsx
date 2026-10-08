import { indentWithTab } from "@codemirror/commands";
import { type Diagnostic, lintGutter, setDiagnostics } from "@codemirror/lint";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { basicSetup } from "codemirror";
import { useEffect, useMemo, useRef } from "react";
import { darkEditorTheme, lightEditorTheme } from "../theme";

export interface Selection {
  from: number;
  to: number;
  /** Changes on every request so that the same range can be selected again. */
  nonce: number;
}

interface Props {
  /** Identifies the document.  Each document keeps its own undo history. */
  docKey: string;
  value: string;
  onChange: (value: string) => void;
  language: Extension;
  dark: boolean;
  diagnostics?: Diagnostic[];
  selection?: Selection | null;
  extensions?: Extension;
  label: string;
}

export function CodeEditor({
  docKey,
  value,
  onChange,
  language,
  dark,
  diagnostics = [],
  selection,
  extensions = [],
  label,
}: Props) {
  const parent = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const states = useRef(new Map<string, EditorState>());
  const currentKey = useRef(docKey);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  const compartments = useMemo(
    () => ({
      language: new Compartment(),
      theme: new Compartment(),
      extra: new Compartment(),
    }),
    [],
  );

  // keep the latest configuration around for newly created states
  const config = useRef({ language, dark, extensions });
  config.current = { language, dark, extensions };

  const createState = (doc: string) =>
    EditorState.create({
      doc,
      extensions: [
        basicSetup,
        keymap.of([indentWithTab]),
        lintGutter(),
        compartments.language.of(config.current.language),
        compartments.theme.of(
          config.current.dark ? darkEditorTheme : lightEditorTheme,
        ),
        compartments.extra.of(config.current.extensions),
        EditorView.contentAttributes.of({ "aria-label": label }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            onChangeRef.current(update.state.doc.toString());
          }
        }),
      ],
    });

  const reconfigure = () => {
    view.current?.dispatch({
      effects: [
        compartments.language.reconfigure(config.current.language),
        compartments.theme.reconfigure(
          config.current.dark ? darkEditorTheme : lightEditorTheme,
        ),
        compartments.extra.reconfigure(config.current.extensions),
      ],
    });
  };

  // create the view
  useEffect(() => {
    view.current = new EditorView({
      state: createState(value),
      parent: parent.current!,
    });
    return () => {
      view.current?.destroy();
      view.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // switch documents
  useEffect(() => {
    const v = view.current;
    if (!v || currentKey.current === docKey) {
      return;
    }
    states.current.set(currentKey.current, v.state);
    currentKey.current = docKey;
    const saved = states.current.get(docKey);
    v.setState(saved ?? createState(value));
    reconfigure();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [docKey]);

  // apply external changes
  useEffect(() => {
    const v = view.current;
    if (v && v.state.doc.toString() !== value) {
      v.dispatch({
        changes: { from: 0, to: v.state.doc.length, insert: value },
      });
    }
  }, [value, docKey]);

  useEffect(reconfigure, [language, dark, extensions, compartments]);

  useEffect(() => {
    const v = view.current;
    if (v) {
      const length = v.state.doc.length;
      const clamped = diagnostics.map((d) => ({
        ...d,
        from: Math.min(d.from, length),
        to: Math.min(d.to, length),
      }));
      v.dispatch(setDiagnostics(v.state, clamped));
    }
  }, [diagnostics, docKey]);

  useEffect(() => {
    const v = view.current;
    if (v && selection) {
      const length = v.state.doc.length;
      v.dispatch({
        selection: {
          anchor: Math.min(selection.from, length),
          head: Math.min(selection.to, length),
        },
        scrollIntoView: true,
      });
      v.focus();
    }
  }, [selection]);

  return <div className="editor" ref={parent} />;
}
