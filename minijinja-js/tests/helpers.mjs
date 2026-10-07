import assert from "node:assert/strict";

/** Smoke tests the exports of an entry point. */
export function checkEntryPoint({
  Environment,
  SafeString,
  TemplateError,
  markSafe,
}) {
  const env = new Environment();
  assert.equal(
    env.renderNamedStr("x.html", "{{ a }}{{ b }}", {
      a: markSafe("<a>"),
      b: "<b>",
    }),
    "<a>&lt;b&gt;",
  );
  assert.ok(markSafe("x") instanceof SafeString);
  assert.throws(() => env.renderStr("{{ x }"), TemplateError);
}
