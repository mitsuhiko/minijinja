/**
 * The default config code.  All examples are commented out so that they can
 * be enabled by uncommenting them.
 */
export const DEFAULT_CONFIG = `// This code runs before every render.  The environment is available as
// \`env\`, the helpers passState, markSafe, SafeString and TemplateError are
// in scope.  Uncomment any of the examples below to try them.  Output of
// console.log() and friends shows up in the Console tab.

// -- Filters, tests and functions ------------------------------------------

// A filter receives the value and the arguments of the filter.
// env.addFilter("shout", (value) => String(value).toUpperCase() + "!");

// Keyword arguments are passed as a trailing object.
// env.addFilter("repeat", (value, times = 2, { sep = "" } = {}) =>
//   Array(times).fill(value).join(sep));

// A test returns true or false:  {% if user is admin %}
// env.addTest("admin", (value) => value?.role === "admin");

// A global function:  {{ greet("World") }}
// env.addFunction("greet", (name) => \`Hello \${name}!\`);

// Log values while rendering:  {{ user|log }}
// env.addFilter("log", (value, ...args) => {
//   console.log(value, ...args);
//   return value;
// });

// A global value (functions and objects work too).
// env.addGlobal("site", { name: "My Site", url: "https://example.com" });

// Built-in filters, tests and globals can be removed.
// env.removeFilter("upper");
// env.removeTest("odd");
// env.removeGlobal("range");

// -- Safe strings and errors ------------------------------------------------

// markSafe (or new SafeString) marks output as safe from auto escaping.
// env.addFilter("bold", (value) => markSafe(\`<b>\${value}</b>\`));

// Safe values are passed to callbacks as SafeString.
// env.addTest("safe", (value) => value instanceof SafeString);

// Throw a TemplateError to fail with a specific kind and message.
// env.addFilter("sqrt", (value) => {
//   if (value < 0) {
//     throw new TemplateError("value must not be negative", { kind: "InvalidOperation" });
//   }
//   return Math.sqrt(value);
// });

// -- Engine state -------------------------------------------------------------

// passState passes the engine state as first argument.  It can look up
// variables and inspect the template and its auto escaping.
// env.addFunction("whoami", passState((state) =>
//   \`\${state.name} (escaping: \${state.autoEscape}, block: \${state.currentBlock})\`));
// env.addFilter("greeting", passState((state, name) =>
//   \`\${state.lookup("greeting") ?? "Hello"} \${name}\`));

// The state can also apply filters and perform tests.
// env.addFilter("shout_title", passState((state, value) =>
//   state.applyFilter("title", value) + (state.performTest("odd", 1) ? "!" : "")));

// -- Rendering --------------------------------------------------------------

// A finalizer customizes values before they are printed (return undefined
// to print the value as is).
// env.setFinalizer((value) => (value === null ? "" : undefined));

// Decide auto escaping per template name ("html", "json", "none", true or false).
// env.setAutoEscapeCallback((name) => name.endsWith(".html") || name.endsWith(".htm"));

// Resolve relative includes such as {% include "./partial.html" %}.
// env.setPathJoinCallback((name, parent) =>
//   name.startsWith("./") ? parent.replace(/[^/]*$/, "") + name.slice(2) : name);

// -- Settings (the same as in the settings menu) ------------------------------

// env.pycompat = true;
// env.trimBlocks = true;
// env.lstripBlocks = true;
// env.keepTrailingNewline = true;
// env.undefinedBehavior = "strict";  // "lenient", "chainable", "semi_strict"
// env.fuel = 10000;                  // limits the work a template can do
// env.syntax = { variableStart: "\${", variableEnd: "}" };
`;
