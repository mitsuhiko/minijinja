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

    it("should return a map when dictionary literals are used", () => {
      const env = new Environment();
      const result = env.evalExpr("{'a': 1, 'b': n}", { n: 2 });
      assert(result instanceof Map);
      let obj = Object.fromEntries(result);
      expect(obj).to.deep.equal({ a: 1, b: 2 });
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
        /loader threw error: /
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
      expect(result).to.equal("true");
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
      expect(result).to.equal("[[1, 2]]");
    });
  });
});
