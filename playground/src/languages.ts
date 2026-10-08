import {
  type Completion,
  type CompletionContext,
  completeFromList,
} from "@codemirror/autocomplete";
import { javascript, javascriptLanguage } from "@codemirror/lang-javascript";
import type { Extension } from "@codemirror/state";
import { placeholder } from "@codemirror/view";
import { html } from "@codemirror/lang-html";
import { jinja } from "@codemirror/lang-jinja";
import { json } from "@codemirror/lang-json";
import { LanguageSupport, StreamLanguage } from "@codemirror/language";
import { extension, stripTemplateExtension } from "./utils";

const plainText = new LanguageSupport(
  StreamLanguage.define({
    name: "text",
    token(stream) {
      stream.skipToEnd();
      return null;
    },
  }),
);

function baseLanguage(fileName: string): LanguageSupport {
  switch (extension(stripTemplateExtension(fileName))) {
    case "html":
    case "htm":
    case "xml":
      return html();
    case "json":
    case "json5":
      return json();
    default:
      return plainText;
  }
}

function describe(value: unknown): string {
  if (Array.isArray(value)) {
    return "array";
  } else if (value === null) {
    return "none";
  }
  return typeof value;
}

function completionsFor(value: unknown, type: string): Completion[] {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return [];
  }
  return Object.entries(value).map(([key, item]) => ({
    label: key,
    type,
    detail: describe(item),
  }));
}

/**
 * Creates the Jinja language support for a template file.
 *
 * Top-level context variables are completed as variables, nested ones are
 * resolved from the live context through `getContext`.
 */
export function templateLanguage(
  fileName: string,
  context: unknown,
  getContext: () => unknown,
): LanguageSupport {
  return jinja({
    base: baseLanguage(fileName),
    variables: completionsFor(context, "variable"),
    properties: (path) => {
      let value = getContext();
      for (const segment of path) {
        while (Array.isArray(value)) {
          value = value[0];
        }
        if (typeof value !== "object" || value === null) {
          return [];
        }
        value = (value as Record<string, unknown>)[segment];
      }
      while (Array.isArray(value)) {
        value = value[0];
      }
      return completionsFor(value, "property");
    },
  });
}

const ENV_COMPLETIONS: Completion[] = [
  ["addFilter", "(name, (value, ...args) => any)"],
  ["addTest", "(name, (value, ...args) => boolean)"],
  ["addFunction", "(name, (...args) => any)"],
  ["addGlobal", "(name, value)"],
  ["removeFilter", "(name)"],
  ["removeTest", "(name)"],
  ["removeGlobal", "(name)"],
  ["setFinalizer", "((value) => any)"],
  ["setAutoEscapeCallback", "((name) => AutoEscape)"],
  ["debug", "boolean"],
  ["pycompat", "boolean"],
  ["trimBlocks", "boolean"],
  ["lstripBlocks", "boolean"],
  ["keepTrailingNewline", "boolean"],
  ["undefinedBehavior", '"lenient" | "chainable" | "semi_strict" | "strict"'],
  ["fuel", "number | null"],
  ["syntax", "SyntaxConfig"],
].map(([label, detail]) => ({
  label,
  detail,
  type: detail.startsWith("(") ? "method" : "property",
}));

const HELPER_COMPLETIONS: Completion[] = [
  { label: "env", type: "variable", detail: "Environment" },
  { label: "passState", type: "function", detail: "(fn) => fn" },
  { label: "markSafe", type: "function", detail: "(value) => SafeString" },
  { label: "SafeString", type: "class" },
  { label: "TemplateError", type: "class" },
];

const completeEnvMembers = completeFromList(ENV_COMPLETIONS);
const completeHelpers = completeFromList(HELPER_COMPLETIONS);

function configCompletions(context: CompletionContext) {
  if (context.matchBefore(/\benv\.\w*$/)) {
    return completeEnvMembers(context);
  }
  return completeHelpers(context);
}

/** Language support for the config code. */
export function configLanguage(): Extension {
  return [
    javascript(),
    javascriptLanguage.data.of({ autocomplete: configCompletions }),
    placeholder(
      "// JavaScript that runs before every render with the environment as `env`.\n" +
        "// Also available: passState, markSafe, SafeString and TemplateError.\n" +
        "//\n" +
        '// env.addFilter("shout", (value) => String(value).toUpperCase() + "!");',
    ),
  ];
}
