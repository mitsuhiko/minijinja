import type { Completion } from "@codemirror/autocomplete";
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
