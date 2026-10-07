import assert from "assert";
import { expect } from "chai";
import path from "node:path";
import { Environment } from "../dist/node/minijinja_js.js";

describe("minijinja-js", () => {
  describe("basic", () => {
    it("should render a basic template", () => {
      const env = new Environment();
      env.addTemplate("test", "Hello, {{ name }}!");
      const result = env.renderTemplate("test", { name: "World" });
      expect(result).to.equal("Hello, World!");
    });

    it("should fail with errors on bad syntax", () => {
      const env = new Environment();
      expect(() => env.addTemplate("test", "Hello, {{ name }")).to.throw(
        "syntax error: unexpected `}`, expected end of variable block"
      );
    });

    it("should use auto escaping for html files", () => {
      const env = new Environment();
      env.addTemplate("test.html", "Hello, {{ name }}!");
      const result = env.renderTemplate("test.html", { name: "<b>World</b>" });
      expect(result).to.equal("Hello, &lt;b&gt;World&lt;&#x2f;b&gt;!");
    });

    it("should not use auto escaping for txt files", () => {
      const env = new Environment();
      env.addTemplate("test.txt", "Hello, {{ name }}!");
      const result = env.renderTemplate("test.txt", { name: "<b>World</b>" });
      expect(result).to.equal("Hello, <b>World</b>!");
    });

    it("should use Python-compatible collection rendering", () => {
      const env = new Environment();
      expect(env.renderStr("{{ [name, true, none] }}", { name: "World" }))
        .to.equal("['World', True, None]");
      expect(env.renderStr("{{ (name, true) }}", { name: "World" }))
        .to.equal("('World', True)");
    });

    it("should use Jinja-compatible JSON spacing", () => {
      const env = new Environment();
      const result = env.renderStr("{{ value|tojson }}", {
        value: { a: 1, b: [2, 3] },
      });
      expect(result).to.equal('{"a": 1, "b": [2, 3]}');
    });

    it("should support semi-strict undefined behavior", () => {
      const env = new Environment();
      env.undefinedBehavior = "semi_strict";
      expect(env.undefinedBehavior).to.equal("semi_strict");
    });

    it("should reject invalid undefined behaviors", () => {
      const env = new Environment();
      expect(() => {
        env.undefinedBehavior = "bogus";
      }).to.throw("invalid undefined behavior");
      expect(env.undefinedBehavior).to.equal("lenient");
      expect(env.renderStr("{{ 1 }}")).to.equal("1");
    });
  });

  describe("debug", () => {
    it("should print the template in the error context", () => {
      const env = new Environment();
      env.debug = true;
      expect(() => env.addTemplate("test", "Hello, {{ name }")).to.throw(
        `syntax error: unexpected \`}\`, expected end of variable block (in test:1)
------------------------------------ test -------------------------------------
   1 > Hello, {{ name }
     i                ^ syntax error
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
No referenced variables
-------------------------------------------------------------------------------`
      );
    });
  });

  describe("eval", () => {
    it("should evaluate an expression", () => {
      const env = new Environment();
      const result = env.evalExpr("1 + 1", {});
      expect(result).to.equal(2);
    });

    it("should fail with errors on bad syntax", () => {
      const env = new Environment();
      expect(() => env.evalExpr("1 +")).to.throw(
        "syntax error: unexpected end of input, expected expression"
      );
    });

    it("should return objects for dictionaries with string keys", () => {
      const env = new Environment();
      const result = env.evalExpr("{'a': 1, 'b': n}", { n: 2 });
      expect(Object.getPrototypeOf(result)).to.equal(Object.prototype);
      expect(result).to.deep.equal({ a: 1, b: 2 });
    });

    it("should return maps for dictionaries with other keys", () => {
      const env = new Environment();
      const result = env.evalExpr("{1: 'a', 'b': 2}");
      assert(result instanceof Map);
      expect([...result]).to.deep.equal([[1, "a"], ["b", 2]]);
    });

    it("should not invoke the __proto__ setter", () => {
      const env = new Environment();
      const result = env.evalExpr("{'__proto__': {'polluted': true}}");
      expect(Object.getPrototypeOf(result)).to.equal(Object.prototype);
      expect(Object.keys(result)).to.deep.equal(["__proto__"]);
    });

    it("should map none and undefined", () => {
      const env = new Environment();
      expect(env.evalExpr("none")).to.equal(null);
      expect(env.evalExpr("missing")).to.equal(undefined);
    });

    it("should convert large integers to bigints", () => {
      const env = new Environment();
      expect(env.evalExpr("2 ** 53 - 1")).to.equal(Number.MAX_SAFE_INTEGER);
      expect(env.evalExpr("x * 1000", { x: 2 ** 52 })).to.equal(
        2n ** 52n * 1000n
      );
      expect(env.evalExpr("1.5 + 1")).to.equal(2.5);
    });

    it("should return wrapped JavaScript values unchanged", () => {
      const env = new Environment();
      const func = () => 42;
      const obj = new (class Foo {})();
      expect(env.evalExpr("func", { func })).to.equal(func);
      expect(env.evalExpr("obj", { obj })).to.equal(obj);
    });

    it("should return tuples as arrays", () => {
      const env = new Environment();
      expect(env.evalExpr("(1, name)", { name: "World" })).to.deep.equal([
        1,
        "World",
      ]);
    });

    it("should allow passing of functions to templates", () => {
      const env = new Environment();
      const result = env.evalExpr("hello()", { hello: () => "World" });
      expect(result).to.equal("World");
    });

    it("should allow passing of functions to templates, even in arrays", () => {
      const env = new Environment();
      const result = env.evalExpr("hello[0]()", { hello: [() => "World"] });
      expect(result).to.equal("World");
    });
  });

  describe("filters", () => {
    it("should add a filter", () => {
      const env = new Environment();
      env.addFilter("my_reverse", (value) =>
        value.split("").reverse().join("")
      );
      const result = env.renderStr("{{ 'hello'|my_reverse }}", {});
      expect(result).to.equal("olleh");
    });

    it("should pass keyword arguments as trailing object", () => {
      const env = new Environment();
      env.addFilter("args", (...args) => JSON.stringify(args));
      const result = env.renderStr("{{ 1|args(2, x=3) }}");
      expect(result).to.equal('[1,2,{"x":3}]');
    });

    it("should pass maps as plain objects", () => {
      const env = new Environment();
      env.addFilter("keys", (value) => Object.keys(value).join(","));
      expect(env.renderStr("{{ {'b': 1, 'a': 2}|keys }}")).to.equal("b,a");
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
      expect(caught).to.be.instanceOf(Error);
      expect(caught.message).to.contain(
        "JavaScript function threw: TypeError: kaboom"
      );
      expect(caught.cause).to.equal(error);
      expect(env.renderStr("{{ 42 }}")).to.equal("42");
    });

    it("should allow rendering from within a filter", () => {
      const env = new Environment();
      env.addFilter("wrap", (value) => env.renderStr("[{{ value }}]", { value }));
      expect(env.renderStr("{{ 1|wrap }}")).to.equal("[1]");
    });

    it("should reject modifications of the environment while rendering", () => {
      const env = new Environment();
      env.addFilter("mutate", (value) => {
        env.debug = true;
        return value;
      });
      expect(() => env.renderStr("{{ 1|mutate }}")).to.throw(
        "cannot modify the environment while it is in use"
      );
      env.debug = true;
      expect(env.debug).to.equal(true);
      expect(env.renderStr("{{ 1 }}")).to.equal("1");
    });
  });

  describe("values", () => {
    it("should convert undefined and null", () => {
      const env = new Environment();
      expect(
        env.renderStr("{{ a is undefined }} {{ b is none }}", {
          a: undefined,
          b: null,
        })
      ).to.equal("True True");
    });

    it("should preserve object key order", () => {
      const env = new Environment();
      const result = env.renderStr("{{ obj|items|list }}", {
        obj: { z: 1, a: 2 },
      });
      expect(result).to.equal("[('z', 1), ('a', 2)]");
    });

    it("should convert maps, sets and dates", () => {
      const env = new Environment();
      const result = env.renderStr("{{ m.a }} {{ s }} {{ d }}", {
        m: new Map([["a", 1]]),
        s: new Set([1, 2]),
        d: new Date(0),
      });
      expect(result).to.equal("1 [1, 2] 1970-01-01T00:00:00.000Z");
    });

    it("should convert bigints and bytes", () => {
      const env = new Environment();
      expect(env.renderStr("{{ x }}", { x: 10n ** 20n })).to.equal(
        "100000000000000000000"
      );
      const bytes = env.evalExpr("b", { b: new Uint8Array([1, 2]) });
      assert(bytes instanceof Uint8Array);
      expect([...bytes]).to.deep.equal([1, 2]);
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
      expect(
        env.renderStr(
          "{{ user.name }}|{{ user.greet('Paul') }}|{{ user.missing is undefined }}",
          { user }
        )
      ).to.equal("Peter|Peter greets Paul|True");
      expect(() => env.renderStr("{{ user.broken }}", { user })).to.throw(
        "broken getter"
      );
    });

    it("should reject cyclic structures", () => {
      const env = new Environment();
      const obj = {};
      obj.self = obj;
      expect(() => env.renderStr("{{ obj }}", { obj })).to.throw(
        "nested too deeply"
      );
      expect(env.renderStr("{{ 1 }}")).to.equal("1");
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
      expect(result).to.equal("Hello [include: World]!");
    });

    it("should propagate loader errors", () => {
      const env = new Environment();
      env.setLoader((_name) => {
        throw new Error("boom");
      });
      env.addTemplate("main.html", "{% include 'x' %}");
      expect(() => env.renderTemplate("main.html", {})).to.throw(
        /template loader threw: Error: boom/
      );
    });

    it("should error on invalid return types", () => {
      const env = new Environment();
      env.setLoader((_name) => 42);
      env.addTemplate("main.html", "{% include 'x' %}");
      expect(() => env.renderTemplate("main.html", {})).to.throw(
        "loader must return a string or null/undefined"
      );
    });
  });

  describe("path join", () => {
    it("should join relative include paths", () => {
      const env = new Environment();
      env.setPathJoinCallback((name, parent) => {
        const joined = path.join(path.dirname(parent), name);
        // Normalize to forward slashes so test is platform-independent
        return joined.replace(/\\\\/g, '/');
      });
      env.setLoader((name) => {
        if (name === "dir/inc.html") return "[{{ value }}]";
        return null;
      });
      env.addTemplate("dir/main.html", "Hello {% include './inc.html' %}!");
      const rv = env.renderTemplate("dir/main.html", { value: "World" });
      expect(rv).to.equal("Hello [World]!");
    });
  });
  describe("tests", () => {
    it("should add a test", () => {
      const env = new Environment();
      env.addTest("hello", (x) => x == "hello");
      const result = env.renderStr("{{ 'hello' is hello }}", {});
      expect(result).to.equal("True");
    });

    it("should pass keyword arguments to tests", () => {
      const env = new Environment();
      env.addTest("between", (x, { lo, hi }) => x >= lo && x <= hi);
      const result = env.renderStr("{{ 5 is between(lo=1, hi=10) }}");
      expect(result).to.equal("True");
    });
  });

  describe("globals", () => {
    it("should allow adding of globals", () => {
      const env = new Environment();
      env.addGlobal("hello", "world");
      const result = env.renderStr("{{ hello }}", {});
      expect(result).to.equal("world");
    });

    it("should allow removing of globals", () => {
      const env = new Environment();
      env.addGlobal("hello", "world");
      env.removeGlobal("hello");
      const result = env.renderStr("{{ hello }}", {});
      expect(result).to.equal("");
    });

    it("should allow adding of globals with a function", () => {
      const env = new Environment();
      env.addGlobal("hello", () => "world");
      const result = env.renderStr("{{ hello() }}", {});
      expect(result).to.equal("world");
    });
  });

  describe("py compat", () => {
    it("should enable py compat", () => {
      const env = new Environment();
      env.enablePyCompat();
      const result = env.renderStr("{{ {1: 2}.items() }}", {});
      expect(result).to.equal("[(1, 2)]");
    });
  });
});
