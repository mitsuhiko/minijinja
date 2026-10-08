// Editor themes and syntax highlighting derived from the MiniJinja logo:
// charcoal and light gray from the gate and kanji, rust orange from the
// lettering.  Orange marks Jinja syntax, everything else stays in the
// neutral tones.  All colors are CSS variables (see styles.css) so that
// switching between light and dark only changes CSS.
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags as t } from "@lezer/highlight";

const v = (name: string) => `var(--hl-${name})`;

/** Highlighting shared by all editors (templates, base languages and JSON). */
const baseHighlight = HighlightStyle.define([
  {
    tag: [
      t.keyword,
      t.controlKeyword,
      t.definitionKeyword,
      t.operatorKeyword,
      t.modifier,
    ],
    color: v("keyword"),
    fontWeight: "600",
  },
  {
    tag: [
      t.logicOperator,
      t.operator,
      t.arithmeticOperator,
      t.compareOperator,
      t.definitionOperator,
    ],
    color: v("operator"),
  },
  { tag: t.variableName, color: v("variable") },
  {
    tag: [t.definition(t.variableName), t.standard(t.variableName), t.self],
    color: v("definition"),
  },
  { tag: t.special(t.variableName), color: v("filter") },
  { tag: t.propertyName, color: v("property") },
  { tag: [t.string, t.attributeValue], color: v("string") },
  { tag: [t.number, t.bool, t.null], color: v("number") },
  {
    tag: [t.comment, t.blockComment, t.lineComment],
    color: v("comment"),
    fontStyle: "italic",
  },
  { tag: t.tagName, color: v("tag") },
  { tag: t.angleBracket, color: v("angle") },
  { tag: t.attributeName, color: v("attribute") },
  {
    tag: [
      t.punctuation,
      t.paren,
      t.squareBracket,
      t.derefOperator,
      t.separator,
    ],
    color: v("punctuation"),
  },
  { tag: t.invalid, color: v("invalid") },
]);

/**
 * Highlighting for Jinja delimiters.  This is only used for templates
 * because JSON braces use the same highlighting tag.  As braces are also
 * punctuation (which the base style colors), the rule needs to win over
 * the base style regardless of the order of the style sheets.
 */
export const jinjaHighlight = syntaxHighlighting(
  HighlightStyle.define([
    {
      tag: t.brace,
      color: `${v("delimiter")} !important`,
      fontWeight: "700",
    },
  ]),
);

const editorTheme = (dark: boolean) =>
  EditorView.theme(
    {
      "&": {
        height: "100%",
        fontSize: "var(--code-font-size)",
        backgroundColor: "var(--editor-bg)",
        color: "var(--hl-text)",
      },
      "&.cm-focused": { outline: "none" },
      ".cm-scroller": { fontFamily: "var(--code-font)", lineHeight: "1.55" },
      ".cm-content": { padding: "10px 0", caretColor: "var(--accent)" },
      ".cm-cursor, .cm-dropCursor": {
        borderLeftColor: "var(--accent)",
        borderLeftWidth: "2px",
      },
      ".cm-gutters": {
        border: "none",
        backgroundColor: "var(--editor-bg)",
        color: "var(--text-faint)",
      },
      ".cm-activeLine": { backgroundColor: "var(--editor-active-line)" },
      ".cm-activeLineGutter": {
        backgroundColor: "var(--editor-active-line)",
        color: "var(--text-muted)",
      },
      "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
        { backgroundColor: "var(--editor-selection)" },
      ".cm-selectionMatch": { backgroundColor: "var(--editor-match)" },
      "&.cm-focused .cm-matchingBracket": {
        backgroundColor: "var(--editor-match)",
        outline: "1px solid var(--accent-muted)",
      },
      "&.cm-focused .cm-nonmatchingBracket": { color: "var(--error-text)" },
      ".cm-searchMatch": {
        backgroundColor: "var(--editor-match)",
        outline: "1px solid var(--accent-muted)",
      },
      ".cm-searchMatch.cm-searchMatch-selected": {
        backgroundColor: "var(--editor-selection)",
      },
      ".cm-foldPlaceholder": {
        backgroundColor: "var(--hover)",
        border: "none",
        color: "var(--text-muted)",
      },
      ".cm-tooltip": {
        backgroundColor: "var(--panel)",
        border: "1px solid var(--border)",
        borderRadius: "6px",
        color: "var(--text)",
        boxShadow: "var(--shadow)",
      },
      ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
        backgroundColor: "var(--accent)",
        color: "var(--accent-text)",
      },
      ".cm-completionDetail": {
        color: "var(--text-faint)",
        fontStyle: "normal",
      },
      ".cm-completionMatchedText": {
        textDecoration: "none",
        color: "var(--accent)",
      },
      ".cm-tooltip-autocomplete > ul > li[aria-selected] .cm-completionMatchedText":
        {
          color: "inherit",
        },
      ".cm-panels": {
        backgroundColor: "var(--panel)",
        color: "var(--text)",
        borderColor: "var(--border)",
      },
      ".cm-panels input, .cm-panels button": { color: "inherit" },
      ".cm-textfield": {
        backgroundColor: "var(--bg)",
        border: "1px solid var(--border)",
        borderRadius: "4px",
      },
      ".cm-button": {
        backgroundImage: "none",
        backgroundColor: "var(--hover)",
        border: "1px solid var(--border)",
        borderRadius: "4px",
      },
      ".cm-diagnostic-error": { borderLeftColor: "var(--error-text)" },
      ".cm-lintRange-error": {
        backgroundImage: "none",
        textDecoration: "underline wavy var(--error-text)",
        textUnderlineOffset: "3px",
      },
    },
    { dark },
  );

export const lightEditorTheme = [
  editorTheme(false),
  syntaxHighlighting(baseHighlight),
];
export const darkEditorTheme = [
  editorTheme(true),
  syntaxHighlighting(baseHighlight),
];
