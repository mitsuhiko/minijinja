import assert from "node:assert/strict";
import path from "node:path";
import { describe, it } from "node:test";
import {
  Environment,
  SafeString,
  TemplateError,
  markSafe,
  passState,
} from "minijinja-js";

/** Asserts that `fn` throws an error whose message contains `expected`. */
function assertThrows(fn, expected) {
  assert.throws(fn, (err) => {
    if (expected instanceof RegExp) {
      assert.match(err.message, expected);
    } else {
      assert.ok(
        err.message.includes(expected),
        `expected ${JSON.stringify(err.message)} to contain ${JSON.stringify(expected)}`,
      );
    }
    return true;
  });
}

describe("minijinja-js", () => {
  describe("basic", () => {
    it("should render a basic template", () => {
      const env = new Environment();
      env.addTemplate("test", "Hello, {{ name }}!");
      const result = env.renderTemplate("test", { name: "World" });
      assert.equal(result, "Hello, World!");
    });

    it("should fail with errors on bad syntax", () => {
      const env = new Environment();
      assertThrows(
        () => env.addTemplate("test", "Hello, {{ name }"),
        "syntax error: unexpected `}`, expected end of variable block",
      );
    });

    it("should use auto escaping for html files", () => {
      const env = new Environment();
      env.addTemplate("test.html", "Hello, {{ name }}!");
      const result = env.renderTemplate("test.html", { name: "<b>World</b>" });
      assert.equal(result, "Hello, &lt;b&gt;World&lt;&#x2f;b&gt;!");
    });

    it("should not use auto escaping for txt files", () => {
      const env = new Environment();
      env.addTemplate("test.txt", "Hello, {{ name }}!");
      const result = env.renderTemplate("test.txt", { name: "<b>World</b>" });
      assert.equal(result, "Hello, <b>World</b>!");
    });

    it("should use Python-compatible collection rendering", () => {
      const env = new Environment();
      assert.equal(
        env.renderStr("{{ [name, true, none] }}", { name: "World" }),
        "['World', True, None]",
      );
      assert.equal(
        env.renderStr("{{ (name, true) }}", { name: "World" }),
        "('World', True)",
      );
    });

    it("should use Jinja-compatible JSON spacing", () => {
      const env = new Environment();
      const result = env.renderStr("{{ value|tojson }}", {
        value: { a: 1, b: [2, 3] },
      });
      assert.equal(result, '{"a": 1, "b": [2, 3]}');
    });

    it("should support semi-strict undefined behavior", () => {
      const env = new Environment();
      env.undefinedBehavior = "semi_strict";
      assert.equal(env.undefinedBehavior, "semi_strict");
    });

    it("should reject invalid undefined behaviors", () => {
      const env = new Environment();
      assertThrows(() => {
        env.undefinedBehavior = "bogus";
      }, "invalid undefined behavior");
      assert.equal(env.undefinedBehavior, "lenient");
      assert.equal(env.renderStr("{{ 1 }}"), "1");
    });
  });

  describe("errors", () => {
    it("should raise template errors with details", () => {
      const env = new Environment();
      let caught;
      try {
        env.addTemplate("test.html", "line 1\n{{ name }");
      } catch (err) {
        caught = err;
      }
      assert.ok(caught instanceof TemplateError);
      assert.ok(caught instanceof Error);
      assert.equal(caught.name, "TemplateError");
      assert.equal(caught.kind, "SyntaxError");
      assert.equal(
        caught.detail,
        "unexpected `}`, expected end of variable block",
      );
      assert.equal(caught.templateName, "test.html");
      assert.equal(caught.line, 2);
      assert.deepEqual(caught.range, { start: 15, end: 16 });
    });
  });

  describe("debug", () => {
    it("should print the template in the error context", () => {
      const env = new Environment();
      env.debug = true;
      assertThrows(
        () => env.addTemplate("test", "Hello, {{ name }"),
        `syntax error: unexpected \`}\`, expected end of variable block (in test:1)
------------------------------------ test -------------------------------------
   1 > Hello, {{ name }
     i                ^ syntax error
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
No referenced variables
-------------------------------------------------------------------------------`,
      );
    });
  });

  describe("eval", () => {
    it("should evaluate an expression", () => {
      const env = new Environment();
      const result = env.evalExpr("1 + 1", {});
      assert.equal(result, 2);
    });

    it("should fail with errors on bad syntax", () => {
      const env = new Environment();
      assertThrows(
        () => env.evalExpr("1 +"),
        "syntax error: unexpected end of input, expected expression",
      );
    });

    it("should return objects for dictionaries with string keys", () => {
      const env = new Environment();
      const result = env.evalExpr("{'a': 1, 'b': n}", { n: 2 });
      assert.equal(Object.getPrototypeOf(result), Object.prototype);
      assert.deepEqual(result, { a: 1, b: 2 });
    });

    it("should return maps for dictionaries with other keys", () => {
      const env = new Environment();
      const result = env.evalExpr("{1: 'a', 'b': 2}");
      assert.ok(result instanceof Map);
      assert.deepEqual(
        [...result],
        [
          [1, "a"],
          ["b", 2],
        ],
      );
    });

    it("should not invoke the __proto__ setter", () => {
      const env = new Environment();
      const result = env.evalExpr("{'__proto__': {'polluted': true}}");
      assert.equal(Object.getPrototypeOf(result), Object.prototype);
      assert.deepEqual(Object.keys(result), ["__proto__"]);
    });

    it("should map none and undefined", () => {
      const env = new Environment();
      assert.equal(env.evalExpr("none"), null);
      assert.equal(env.evalExpr("missing"), undefined);
    });

    it("should convert large integers to bigints", () => {
      const env = new Environment();
      assert.equal(env.evalExpr("2 ** 53 - 1"), Number.MAX_SAFE_INTEGER);
      assert.equal(env.evalExpr("x * 1000", { x: 2 ** 52 }), 2n ** 52n * 1000n);
      assert.equal(env.evalExpr("1.5 + 1"), 2.5);
    });

    it("should return wrapped JavaScript values unchanged", () => {
      const env = new Environment();
      const func = () => 42;
      const obj = new (class Foo {})();
      assert.equal(env.evalExpr("func", { func }), func);
      assert.equal(env.evalExpr("obj", { obj }), obj);
    });

    it("should return tuples as arrays", () => {
      const env = new Environment();
      assert.deepEqual(env.evalExpr("(1, name)", { name: "World" }), [
        1,
        "World",
      ]);
    });

    it("should allow passing of functions to templates", () => {
      const env = new Environment();
      const result = env.evalExpr("hello()", { hello: () => "World" });
      assert.equal(result, "World");
    });

    it("should allow passing of functions to templates, even in arrays", () => {
      const env = new Environment();
      const result = env.evalExpr("hello[0]()", { hello: [() => "World"] });
      assert.equal(result, "World");
    });
  });

  describe("filters", () => {
    it("should add a filter", () => {
      const env = new Environment();
      env.addFilter("my_reverse", (value) =>
        value.split("").reverse().join(""),
      );
      const result = env.renderStr("{{ 'hello'|my_reverse }}", {});
      assert.equal(result, "olleh");
    });

    it("should pass keyword arguments as trailing object", () => {
      const env = new Environment();
      env.addFilter("args", (...args) => JSON.stringify(args));
      const result = env.renderStr("{{ 1|args(2, x=3) }}");
      assert.equal(result, '[1,2,{"x":3}]');
    });

    it("should pass maps as plain objects", () => {
      const env = new Environment();
      env.addFilter("keys", (value) => Object.keys(value).join(","));
      assert.equal(env.renderStr("{{ {'b': 1, 'a': 2}|keys }}"), "b,a");
    });

    it("should report exceptions and keep the environment usable", () => {
      const env = new Environment();
      const error = new TypeError("kaboom");
      env.addFilter("boom", () => {
        throw error;
      });
      let caught;
      try {
        env.renderStr("{{ 1|boom }}");
      } catch (err) {
        caught = err;
      }
      assert.ok(caught instanceof Error);
      assert.ok(
        caught.message.includes("JavaScript function threw: TypeError: kaboom"),
      );
      assert.equal(caught.cause, error);
      assert.equal(env.renderStr("{{ 42 }}"), "42");
    });

    it("should allow rendering from within a filter", () => {
      const env = new Environment();
      env.addFilter("wrap", (value) =>
        env.renderStr("[{{ value }}]", { value }),
      );
      assert.equal(env.renderStr("{{ 1|wrap }}"), "[1]");
    });

    it("should reject modifications of the environment while rendering", () => {
      const env = new Environment();
      env.addFilter("mutate", (value) => {
        env.debug = true;
        return value;
      });
      assertThrows(
        () => env.renderStr("{{ 1|mutate }}"),
        "cannot modify the environment while it is in use",
      );
      env.debug = true;
      assert.equal(env.debug, true);
      assert.equal(env.renderStr("{{ 1 }}"), "1");
    });
  });

  describe("safe strings", () => {
    it("should not escape safe strings", () => {
      const env = new Environment();
      env.addFilter("bold", (value) => new SafeString(`<b>${value}</b>`));
      const result = env.renderNamedStr(
        "test.html",
        "{{ value }} {{ raw }} {{ 'x'|bold }}",
        { value: "<a>", raw: new SafeString("<a>") },
      );
      assert.equal(result, "&lt;a&gt; <a> <b>x</b>");
    });

    it("should pass safe strings to callbacks", () => {
      const env = new Environment();
      env.addFilter("check", (value) =>
        value instanceof SafeString ? "safe" : "unsafe",
      );
      const result = env.renderStr("{{ 'a'|safe|check }} {{ 'a'|check }}");
      assert.equal(result, "safe unsafe");
      const rv = env.evalExpr("'<a>'|safe");
      assert.ok(rv instanceof SafeString);
      assert.equal(String(rv), "<a>");
    });
  });

  describe("features", () => {
    it("should support loop controls", () => {
      const env = new Environment();
      const result = env.renderStr(
        "{% for x in range(10) %}{% if x == 3 %}{% break %}{% endif %}{{ x }}{% endfor %}",
      );
      assert.equal(result, "012");
    });

    it("should support urlencode", () => {
      const env = new Environment();
      assert.equal(env.renderStr("{{ 'a b&c'|urlencode }}"), "a%20b%26c");
    });
  });

  describe("values", () => {
    it("should convert undefined and null", () => {
      const env = new Environment();
      assert.equal(
        env.renderStr("{{ a is undefined }} {{ b is none }}", {
          a: undefined,
          b: null,
        }),
        "True True",
      );
    });

    it("should preserve object key order", () => {
      const env = new Environment();
      const result = env.renderStr("{{ obj|items|list }}", {
        obj: { z: 1, a: 2 },
      });
      assert.equal(result, "[('z', 1), ('a', 2)]");
    });

    it("should convert maps, sets and dates", () => {
      const env = new Environment();
      const result = env.renderStr("{{ m.a }} {{ s }} {{ d }}", {
        m: new Map([["a", 1]]),
        s: new Set([1, 2]),
        d: new Date(0),
      });
      assert.equal(result, "1 [1, 2] 1970-01-01T00:00:00.000Z");
    });

    it("should convert bigints and bytes", () => {
      const env = new Environment();
      assert.equal(
        env.renderStr("{{ x }}", { x: 10n ** 20n }),
        "100000000000000000000",
      );
      const bytes = env.evalExpr("b", { b: new Uint8Array([1, 2]) });
      assert.ok(bytes instanceof Uint8Array);
      assert.deepEqual([...bytes], [1, 2]);
    });

    it("should access class instances lazily", () => {
      class User {
        constructor(name) {
          this._name = name;
        }
        get name() {
          return this._name;
        }
        greet(other) {
          return `${this.name} greets ${other}`;
        }
        get broken() {
          throw new Error("broken getter");
        }
      }
      const env = new Environment();
      const user = new User("Peter");
      assert.equal(
        env.renderStr(
          "{{ user.name }}|{{ user.greet('Paul') }}|{{ user.missing is undefined }}",
          { user },
        ),
        "Peter|Peter greets Paul|True",
      );
      assertThrows(
        () => env.renderStr("{{ user.broken }}", { user }),
        "broken getter",
      );
    });

    it("should reject cyclic structures", () => {
      const env = new Environment();
      const obj = {};
      obj.self = obj;
      assertThrows(
        () => env.renderStr("{{ obj }}", { obj }),
        "nested too deeply",
      );
      assert.equal(env.renderStr("{{ 1 }}"), "1");
    });
  });

  describe("loader", () => {
    it("should resolve includes via setLoader", () => {
      const env = new Environment();
      env.setLoader((name) => {
        if (name === "inc.html") {
          return "[include: {{ value }}]";
        }
        return null;
      });
      env.addTemplate("main.html", "Hello {% include 'inc.html' %}!");
      const result = env.renderTemplate("main.html", { value: "World" });
      assert.equal(result, "Hello [include: World]!");
    });

    it("should propagate loader errors", () => {
      const env = new Environment();
      env.setLoader((_name) => {
        throw new Error("boom");
      });
      env.addTemplate("main.html", "{% include 'x' %}");
      assertThrows(
        () => env.renderTemplate("main.html", {}),
        /template loader threw: Error: boom/,
      );
    });

    it("should error on invalid return types", () => {
      const env = new Environment();
      env.setLoader((_name) => 42);
      env.addTemplate("main.html", "{% include 'x' %}");
      assertThrows(
        () => env.renderTemplate("main.html", {}),
        "loader must return a string or null/undefined",
      );
    });
  });

  describe("path join", () => {
    it("should join relative include paths", () => {
      const env = new Environment();
      env.setPathJoinCallback((name, parent) => {
        const joined = path.join(path.dirname(parent), name);
        // Normalize to forward slashes so test is platform-independent
        return joined.replace(/\\\\/g, "/");
      });
      env.setLoader((name) => {
        if (name === "dir/inc.html") return "[{{ value }}]";
        return null;
      });
      env.addTemplate("dir/main.html", "Hello {% include './inc.html' %}!");
      const rv = env.renderTemplate("dir/main.html", { value: "World" });
      assert.equal(rv, "Hello [World]!");
    });
  });
  describe("tests", () => {
    it("should add a test", () => {
      const env = new Environment();
      env.addTest("hello", (x) => x == "hello");
      const result = env.renderStr("{{ 'hello' is hello }}", {});
      assert.equal(result, "True");
    });

    it("should pass keyword arguments to tests", () => {
      const env = new Environment();
      env.addTest("between", (x, { lo, hi }) => x >= lo && x <= hi);
      const result = env.renderStr("{{ 5 is between(lo=1, hi=10) }}");
      assert.equal(result, "True");
    });
  });

  describe("globals", () => {
    it("should allow adding of globals", () => {
      const env = new Environment();
      env.addGlobal("hello", "world");
      const result = env.renderStr("{{ hello }}", {});
      assert.equal(result, "world");
    });

    it("should allow removing of globals", () => {
      const env = new Environment();
      env.addGlobal("hello", "world");
      env.removeGlobal("hello");
      const result = env.renderStr("{{ hello }}", {});
      assert.equal(result, "");
    });

    it("should allow adding of globals with a function", () => {
      const env = new Environment();
      env.addGlobal("hello", () => "world");
      const result = env.renderStr("{{ hello() }}", {});
      assert.equal(result, "world");
    });
  });

  describe("api", () => {
    it("should add and remove filters, tests and functions", () => {
      const env = new Environment();
      env.addFilter("double", (x) => x * 2);
      env.addTest("even", (x) => x % 2 === 0);
      env.addFunction(
        "greet",
        (name, { greeting = "Hello" } = {}) => `${greeting} ${name}`,
      );
      assert.equal(
        env.renderStr(
          "{{ 2|double }} {{ 2 is even }} {{ greet('Peter', greeting='Hi') }}",
        ),
        "4 True Hi Peter",
      );
      env.removeFilter("double");
      env.removeTest("even");
      assertThrows(() => env.renderStr("{{ 2|double }}"), "unknown filter");
      assertThrows(() => env.renderStr("{{ 2 is even }}"), "unknown test");
    });

    it("should support an auto escape callback", () => {
      const env = new Environment();
      env.setAutoEscapeCallback((name) =>
        name.endsWith(".j2") ? "html" : name.endsWith(".json") ? "json" : false,
      );
      const ctx = { value: "<a>" };
      assert.equal(env.renderNamedStr("x.j2", "{{ value }}", ctx), "&lt;a&gt;");
      assert.equal(env.renderNamedStr("x.json", "{{ value }}", ctx), '"<a>"');
      assert.equal(env.renderNamedStr("x.html", "{{ value }}", ctx), "<a>");
    });

    it("should support a finalizer", () => {
      const env = new Environment();
      env.setFinalizer((value) => (value === null ? "" : undefined));
      assert.equal(env.renderStr("[{{ none }}] [{{ 42 }}]"), "[] [42]");
    });

    it("should support custom syntax", () => {
      const env = new Environment();
      assert.deepEqual(env.syntax, {
        blockStart: "{%",
        blockEnd: "%}",
        variableStart: "{{",
        variableEnd: "}}",
        commentStart: "{#",
        commentEnd: "#}",
        lineStatementPrefix: null,
        lineCommentPrefix: null,
      });
      env.trimBlocks = true;
      env.syntax = {
        variableStart: "${",
        variableEnd: "}",
        lineStatementPrefix: "#",
      };
      assert.equal(env.syntax.variableStart, "${");
      assert.equal(env.syntax.blockStart, "{%");
      assert.equal(env.trimBlocks, true);
      assert.equal(
        env.renderStr("# for x in seq\n${ x }{{ x }}\n# endfor\n", {
          seq: [1, 2],
        }),
        "1{{ x }}\n2{{ x }}\n",
      );
      assertThrows(() => {
        env.syntax = { blockStart: 42 };
      }, "must be a string");
    });

    it("should report undeclared variables", () => {
      const env = new Environment();
      env.addTemplate("x", "{% set a = 1 %}{{ a }}{{ b }}{{ user.name }}");
      assert.deepEqual(env.undeclaredVariablesInTemplate("x"), ["b", "user"]);
      assert.deepEqual(env.undeclaredVariablesInTemplate("x", true), [
        "b",
        "user.name",
      ]);
      assert.deepEqual(env.undeclaredVariablesInStr("{{ x }}{{ y }}"), [
        "x",
        "y",
      ]);
    });
  });

  describe("state", () => {
    it("should pass the state to marked callbacks", () => {
      const env = new Environment();
      env.addFilter(
        "info",
        passState(
          (state, value) =>
            `${state.name}:${state.autoEscape}:${state.lookup("greeting")}:${state.lookup("missing")}:${state.applyFilter("upper", value)}:${state.performTest("odd", 3)}`,
        ),
      );
      env.addGlobal("greeting", "hi");
      assert.equal(
        env.renderNamedStr("x.html", "{{ 'a'|info }}"),
        "x.html:html:hi:undefined:A:true",
      );
    });

    it("should pass the state to functions and methods", () => {
      const env = new Environment();
      const fn = passState((state) => state.lookup("x"));
      class Obj {
        method = passState((state) => state.lookup("x") * 2);
      }
      assert.equal(
        env.renderStr("{{ fn() }} {{ obj.method() }}", {
          fn,
          obj: new Obj(),
          x: 21,
        }),
        "21 42",
      );
    });

    it("should invalidate the state after the callback", () => {
      const env = new Environment();
      let saved;
      env.addFilter(
        "keep",
        passState((state) => {
          saved = state;
          return 1;
        }),
      );
      env.renderStr("{{ 1|keep }}");
      assertThrows(() => saved.lookup("x"), "only be used while the callback");
    });

    it("should map thrown template errors", () => {
      const env = new Environment();
      env.addFilter("fail", () => {
        throw new TemplateError("value must be positive", {
          kind: "InvalidOperation",
        });
      });
      env.addFilter("missing", () => {
        throw new TemplateError("need more", { kind: "MissingArgument" });
      });
      assertThrows(
        () => env.renderStr("{{ 1|fail }}"),
        /^invalid operation: value must be positive/,
      );
      let caught;
      try {
        env.renderStr("{{ 1|missing }}");
      } catch (err) {
        caught = err;
      }
      assert.equal(caught.kind, "MissingArgument");
      assert.equal(caught.detail, "need more");
    });
  });

  describe("rand", () => {
    it("should provide random functions", () => {
      const env = new Environment();
      const values = new Set();
      for (let i = 0; i < 5; i++) {
        values.add(env.renderStr("{{ randrange(1000000000) }}"));
      }
      assert.ok(values.size > 1);
      assert.match(env.renderStr("{{ [1, 2, 3]|random }}"), /^[123]$/);
      assert.match(env.renderStr("{{ lipsum(1) }}"), /^[A-Z]/);
      assert.equal(
        env.renderStr("{{ randrange(1000000) }}", { RAND_SEED: 42 }),
        env.renderStr("{{ randrange(1000000) }}", { RAND_SEED: 42 }),
      );
    });
  });

  describe("py compat", () => {
    it("should enable py compat", () => {
      const env = new Environment();
      env.enablePyCompat();
      const result = env.renderStr("{{ {1: 2}.items() }}", {});
      assert.equal(result, "[(1, 2)]");
    });

    it("should toggle py compat", () => {
      const env = new Environment();
      assert.equal(env.pycompat, false);
      env.pycompat = true;
      assert.equal(env.renderStr("{{ 'abc'.startswith('a') }}"), "True");
      env.pycompat = false;
      assert.equal(env.pycompat, false);
      assertThrows(
        () => env.renderStr("{{ 'abc'.startswith('a') }}"),
        "unknown method",
      );
    });
  });
});
