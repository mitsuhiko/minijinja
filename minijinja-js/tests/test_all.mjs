import assert from "assert";
import { expect } from "chai";
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
  });
});
