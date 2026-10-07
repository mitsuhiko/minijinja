import { indentWithTab } from "@codemirror/commands";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { type Diagnostic, lintGutter, setDiagnostics } from "@codemirror/lint";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView, keymap } from "@codemirror/view";
import { basicSetup } from "codemirror";
import { tags as t } from "@lezer/highlight";
import { useEffect, useMemo, useRef } from "react";

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

const baseTheme = EditorView.theme({
  "&": { height: "100%", fontSize: "var(--code-font-size)" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--code-font)", lineHeight: "1.55" },
  ".cm-content": { padding: "10px 0" },
  ".cm-gutters": { border: "none" },
});

const lightHighlight = HighlightStyle.define([
  { tag: t.brace, color: "#b7410e", fontWeight: "600" },
  {
    tag: [
      t.keyword,
      t.controlKeyword,
      t.definitionKeyword,
      t.operatorKeyword,
      t.logicOperator,
      t.modifier,
    ],
    color: "#8250df",
  },
  { tag: t.variableName, color: "#0550ae" },
  {
    tag: [t.definition(t.variableName), t.standard(t.variableName), t.self],
    color: "#953800",
  },
  { tag: t.special(t.variableName), color: "#116329", fontWeight: "500" },
  { tag: t.propertyName, color: "#0a3069" },
  { tag: t.string, color: "#0a3069" },
  { tag: [t.number, t.bool, t.null], color: "#0550ae" },
  {
    tag: [t.comment, t.blockComment, t.lineComment],
    color: "#6e7781",
    fontStyle: "italic",
  },
  { tag: [t.tagName, t.angleBracket], color: "#116329" },
  { tag: t.attributeName, color: "#6639ba" },
  { tag: t.attributeValue, color: "#0a3069" },
  {
    tag: [
      t.operator,
      t.arithmeticOperator,
      t.compareOperator,
      t.definitionOperator,
    ],
    color: "#cf222e",
  },
]);

const darkHighlight = HighlightStyle.define([
  { tag: t.brace, color: "#f08d5b", fontWeight: "600" },
  { tag: t.special(t.variableName), color: "#98c379", fontWeight: "500" },
]);

const lightTheme = [
  EditorView.theme({
    "&": { backgroundColor: "var(--editor-bg)", color: "var(--text)" },
    ".cm-gutters": {
      backgroundColor: "var(--editor-bg)",
      color: "var(--text-faint)",
    },
    ".cm-activeLineGutter, .cm-activeLine": {
      backgroundColor: "var(--editor-active-line)",
    },
  }),
  syntaxHighlighting(lightHighlight),
];

const darkTheme = [
  syntaxHighlighting(darkHighlight),
  oneDark,
  EditorView.theme(
    {
      "&": { backgroundColor: "var(--editor-bg)" },
      ".cm-gutters": { backgroundColor: "var(--editor-bg)" },
      ".cm-activeLineGutter, .cm-activeLine": {
        backgroundColor: "var(--editor-active-line)",
      },
    },
    { dark: true },
  ),
];

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
        baseTheme,
        compartments.language.of(config.current.language),
        compartments.theme.of(config.current.dark ? darkTheme : lightTheme),
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
          config.current.dark ? darkTheme : lightTheme,
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
