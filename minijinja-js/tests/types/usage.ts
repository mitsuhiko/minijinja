// Type checks for the published type declarations.  This file is never run.
import {
  Environment,
  SafeString,
  TemplateError,
  markSafe,
  type AutoEscape,
  type Context,
  type SyntaxConfig,
  type UndefinedBehavior,
} from "minijinja-js";
import { init, initSync } from "minijinja-js/init";

class User {
  constructor(public name: string) {}
}

{
  using env = new Environment();
  const ctx: Context = { user: new User("Peter") };
  const rendered: string = env.renderStr("{{ user.name }}", ctx);
  env.renderStr("Hello");
  env.renderTemplate("index.html", null);
  env.renderNamedStr("index.html", "{{ x }}", { x: [1, 2] });
  const value: unknown = env.evalExpr("1 + 1");

  env.addFilter("repeat", (value: string, times: number) => value.repeat(times));
  env.addTest("even", (value: number) => value % 2 === 0);
  env.addFunction("now", () => Date.now());
  env.addGlobal("site", { name: "Example" });
  env.setLoader((name) => (name === "index.html" ? "{{ 42 }}" : null));
  env.setPathJoinCallback((name, parent) => `${parent}/${name}`);
  env.setAutoEscapeCallback((name): AutoEscape => name.endsWith(".html"));
  env.setFinalizer((value) => (value === null ? "" : undefined));

  const behavior: UndefinedBehavior = env.undefinedBehavior;
  env.undefinedBehavior = "strict";
  // @ts-expect-error invalid undefined behavior
  env.undefinedBehavior = "bogus";

  env.syntax = { variableStart: "${", variableEnd: "}" };
  const syntax: SyntaxConfig = env.syntax;
  const prefix: string | null = syntax.lineStatementPrefix;

  env.fuel = 1000;
  env.fuel = null;
  const fuel: number | undefined = env.fuel;
  env.pycompat = true;
  env.debug = true;
  env.trimBlocks = true;

  const variables: string[] = env.undeclaredVariablesInStr("{{ x }}", true);

  const safe: SafeString = markSafe("<b>");
  const upper: string = safe.toUpperCase();
  const safe2 = new SafeString("<i>");

  try {
    env.renderStr("{{ x }");
  } catch (err) {
    if (err instanceof TemplateError) {
      const kind: string = err.kind;
      const line: number | undefined = err.line;
      const range: { start: number; end: number } | undefined = err.range;
      const cause: unknown = err.cause;
    }
  }

  void [rendered, value, behavior, prefix, fuel, variables, upper, safe2];
}

await init();
await init(new URL("https://example.com/minijinja_js_bg.wasm"));
initSync(new Uint8Array());
