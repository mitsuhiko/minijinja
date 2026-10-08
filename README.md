<div align="center">
  <img src="https://github.com/mitsuhiko/minijinja/raw/main/artwork/logo.png" alt="" width=320>
  <p><strong>MiniJinja: a powerful template engine with minimal dependencies</strong></p>

[![License](https://img.shields.io/github/license/mitsuhiko/minijinja)](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/d/minijinja.svg)](https://crates.io/crates/minijinja)
[![Documentation](https://docs.rs/minijinja/badge.svg)](https://docs.rs/minijinja)

</div>

> [!IMPORTANT]
> The `main` branch contains development for MiniJinja 3. MiniJinja 2
> maintenance and releases continue on the
> [`minijinja-2`](https://github.com/mitsuhiko/minijinja/tree/minijinja-2)
> branch. Pull requests for changes that apply to MiniJinja 2 should target that
> branch. Changes are merged forward from `minijinja-2` into `main`, never in the
> other direction.

MiniJinja is a powerful but minimal dependency template engine
which is based on the syntax and behavior of the
[Jinja2](https://jinja.palletsprojects.com/) template engine for Python.

It's implemented in [Rust](https://github.com/mitsuhiko/minijinja/tree/main/minijinja) and [Go](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-go) and is also available via WASM for [JavaScript](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-js),
as a [Python extension module](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-py), and as a [command line utility](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cli).

It supports
[a range of features from Jinja2](https://github.com/mitsuhiko/minijinja/blob/main/COMPATIBILITY.md)
including inheritance, filters and more.  The goal is that it should be possible
to use some templates in Rust programs without the fear of pulling in complex
dependencies for a small problem.  Additionally it tries not to re-invent
something but stay in line with prior art to leverage an already existing
ecosystem of editor integrations.  The default build of MiniJinja is entirely
free of dependencies, though some users might want to enable the `serde`
dependency to enable the optional serde support.

```
$ cargo tree
minimal v0.1.0 (examples/minimal)
└── minijinja v3.0.0-alpha.3 (minijinja)
```

Additionally minijinja is also available as an (optionally pre-compiled) command line executable
called [`minijinja-cli`](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cli):

```
$ curl -sSfL https://github.com/mitsuhiko/minijinja/releases/latest/download/minijinja-cli-installer.sh | sh
$ echo "Hello {{ name }}" | minijinja-cli - -Dname=World
Hello World
```

You can play with MiniJinja online [in the browser playground](https://mitsuhiko.github.io/minijinja/)
powered by a WASM build of MiniJinja.

**Goals:**

* [Well documented](https://docs.rs/minijinja), compact API
* Minimal dependencies, reasonable compile times and [decent runtime performance](https://github.com/mitsuhiko/minijinja/tree/main/benchmarks#comparison-results)
* [Stay as close as possible](https://github.com/mitsuhiko/minijinja/blob/main/COMPATIBILITY.md) to Jinja2
* Support for [expression evaluation](https://docs.rs/minijinja/latest/minijinja/struct.Expression.html) which
  allows the use [as a DSL](https://github.com/mitsuhiko/minijinja/tree/main/examples/dsl)
* Optional support for all [`serde`](https://serde.rs) compatible types through the `serde` feature
* [Well tested](https://github.com/mitsuhiko/minijinja/tree/main/minijinja/tests)
* Support for [dynamic runtime objects](https://docs.rs/minijinja/latest/minijinja/value/trait.Object.html) with methods and dynamic attributes
* [Descriptive errors](https://github.com/mitsuhiko/minijinja/tree/main/examples/error)
* Bindings for [JavaScript](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-js),
  [Python](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-py), and [C](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cabi)
* Also available for [Go](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-go)
* Comes with a handy [CLI](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cli)
* [Compiles to WebAssembly](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-js)

## Example

**Example Template:**

```jinja
{% extends "layout.html" %}
{% block body %}
  <p>Hello {{ name }}!</p>
{% endblock %}
```

**Invoking from Rust:**

```rust
use minijinja::{Environment, context};

fn main() {
    let mut env = Environment::new();
    env.add_template("hello.txt", "Hello {{ name }}!").unwrap();
    let template = env.get_template("hello.txt").unwrap();
    println!("{}", template.render(context! { name => "World" }).unwrap());
}
```

## Getting Help

If you are stuck with `MiniJinja`, have suggestions or need help, you can use the
[GitHub Discussions](https://github.com/mitsuhiko/minijinja/discussions).

## Related Crates

* [minijinja-embed](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-embed): provides
  utilities for embedding templates in a binary
* [minijinja-contrib](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-contrib): provides
  additional utilities too specific for the core
* [minijinja-py](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-py): makes MiniJinja
  available to Python
* [minijinja-js](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-js): makes MiniJinja
  available to JavaScript via WASM (for Node and Browser)
* [minijinja-go](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-go): a native Go
  implementation of MiniJinja
* [minijinja-cli](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cli): a command line utility.
* [minijinja-cabi](https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cabi): a C binding to MiniJinja.

## Use Cases and Users

Here are some interesting Open Source users and use cases of MiniJinja.  The examples link directly to where
the engine is used so you can see how it's utilized:

* AI and LLM Inference:
  * **[NVIDIA Dynamo](https://github.com/ai-dynamo/dynamo)** uses it to [render LLM chat templates](https://github.com/ai-dynamo/frontend-crates/blob/8a5cf0495b83233c804303b73d69981e82af0c3b/renderer/src/template/formatters.rs#L426) in its distributed inference serving framework
  * **[Hugging Face Candle](https://github.com/huggingface/candle)** uses it to [render LLM chat templates](https://github.com/huggingface/candle/blob/c68b24997319b8d76e5a4e775dab197c7f26322a/candle-examples/src/chat_template.rs#L153-L170), including in the browser via WASM
  * **[mistral.rs](https://github.com/EricLBuehler/mistral.rs)** uses it to [render LLM chat templates](https://github.com/EricLBuehler/mistral.rs/blob/c834f59fe0b3b020a56cb6a0279a051370554539/mistralrs-core/src/pipeline/chat_template.rs)
  * **[Shepherd Model Gateway](https://lightseek.org/smg)** uses it to [render LLM chat templates](https://github.com/smg-project/smg/blob/84e465ecbfd6b7a43887a2dcfed344227b7b973b/crates/tokenizer/src/chat_template.rs#L884) in its LLM gateway
  * **[LoRAX](https://loraexchange.ai/)** uses it to [render LLM chat templates](https://github.com/predibase/lorax/blob/6a83954b8c6ffd51eb69e7096ee2730d53b903dd/router/src/infer.rs)
  * **[BoundaryML's BAML](https://docs.boundaryml.com/)** uses it to [render LLM prompts](https://github.com/BoundaryML/baml/blob/17123de7ea653f51547576169bb0589d39053edc/engine/baml-lib/jinja/src/lib.rs)
  * **[Worktrunk](https://github.com/max-sixty/worktrunk)** uses it to [render prompts for LLM generated commit messages](https://github.com/max-sixty/worktrunk/blob/7024f9b31f61bb54ec6848011a76069d6504166e/src/llm.rs#L580-L609)

* Data and Processing:
  * **[dbt](https://github.com/dbt-labs/dbt)** builds the Jinja support of its Rust rewrite (dbt v2.0 / Fusion engine) [on a fork of MiniJinja](https://github.com/dbt-labs/dbt/tree/4966a12076764b150dfc68e3c929698193b6c6b9/crates/dbt-jinja)
  * **[Cube](https://cube.dev/docs/product/data-modeling/dynamic/jinja)** uses it [for data modelling](https://github.com/cube-js/cube/tree/db11c121c77c663845242366d3d972b9bc30ae54/packages/cubejs-backend-native/src/template/mj_value)
  * **[PRQL](https://prql-lang.org/)** uses it [to handle DBT style pipelines](https://github.com/PRQL/prql/blob/59fb3cc4b9b6c9e195c928b1ba1134e2c5706ea3/prqlc/prqlc/src/cli/jinja.rs#L21)
  * **[qsv](https://qsv.dathere.com)** uses it [to render templates from CSV files](https://github.com/jqnatividad/qsv/blob/master/src/cmd/template.rs#L2), to [construct payloads to post to web services](https://github.com/jqnatividad/qsv/blob/master/src/cmd/fetchpost.rs#L3) and to [infer Data Dictionaries, Descriptions & Tags or Chat with your data](https://github.com/dathere/qsv/blob/master/src/cmd/describegpt.rs#L2).
  * **[fenic](https://github.com/typedef-ai/fenic)** uses it to [render Jinja templates over DataFrame columns](https://github.com/typedef-ai/fenic/blob/7645b9a7af672717b238e212128d9dfc3aef32ef/rust/src/jinja/render.rs)
  * **[Fluvio](https://github.com/fluvio-community/fluvio)** uses it to [render connector configurations and resolve secrets](https://github.com/fluvio-community/fluvio/blob/52673942c1c7364f36f2e05da972436f761243cd/crates/fluvio-connector-package/src/render/mod.rs#L32-L40)
  * **[Query.Farm MiniJinja](https://query.farm/products/extensions/minijinja/)** uses it to [render templates from DuckDB SQL](https://github.com/Query-farm/minijinja/blob/v1.5/duckdb_minijinja_binding/src/lib.rs).

* Web Frameworks and Documentation:
  * **[Rocket](https://rocket.rs/)** supports it as a [template engine for dynamic templates](https://github.com/rwf2/Rocket/blob/3a54d079aef060a8f732bd04ea54b0581a604087/contrib/dyn_templates/src/engine/minijinja.rs)
  * **[Litestar](https://litestar.dev/)** supports it via MiniJinja for Python as a [template engine plugin](https://github.com/litestar-org/litestar/blob/5cb5a02e23fee12d17cb41cfdd1e8d581ec14291/litestar/plugins/minijinja.py)
  * **[Ultralytics](https://github.com/ultralytics/ultralytics)** uses MiniJinja for Python to [render macros in its documentation](https://github.com/ultralytics/ultralytics/blob/c7d6425dc4be76d6c0904e3f5792f55dfd377f62/docs/build_docs.py#L85-L120)
  * **[sphinx-needs](https://github.com/useblocks/sphinx-needs)** uses MiniJinja for Python to [render templates in its Sphinx extension](https://github.com/useblocks/sphinx-needs/blob/6434aef0d2953b5f9ee5df32e7d5bee866491f04/packages/sphinx-needs/src/sphinx_needs/_jinja.py)

* Packaging and Build Tools:
  * **[rattler-build](https://github.com/prefix-dev/rattler-build)** uses it to [evaluate conda package recipes](https://github.com/prefix-dev/rattler-build/blob/ae0cbd3e602c377b4896c00856b693f4e1f3ddcf/crates/rattler_build_jinja/src/jinja.rs#L778)
  * **[pixi](https://pixi.sh/)** uses it to [template task commands and arguments](https://github.com/prefix-dev/pixi/blob/f1a706d965b4bb64adfd91d84be66085bfaf9e59/crates/pixi_manifest/src/task.rs#L522)
  * **[tract](https://github.com/sonos/tract)** uses it to [generate SIMD assembly kernels at build time](https://github.com/sonos/tract/blob/f66b50e9ba117f26de70fc125e3a4f665ca1b6fa/linalg/build.rs#L771-L790)

* Developer Tools:
  * **[Atuin](https://atuin.sh/)** uses it to [template shell scripts](https://github.com/atuinsh/atuin/blob/f889a24e02eaec60fca0f1381733207d4e83ed92/crates/atuin-scripts/src/execution.rs#L46-L64)
  * **[HawkEye](https://github.com/fast/hawkeye)** uses it to [render license headers](https://github.com/fast/hawkeye/blob/12072370b89c4472ff8074e9e99582de1bfb696e/hawkeye/src/template.rs#L179)
  * **[MITRE's Hipcheck](https://github.com/mitre/hipcheck)** uses it to [render human readable reports](https://github.com/mitre/hipcheck/blob/09cca775e663ce097dcf2e999b356c5ef3daf86f/hipcheck/src/shell/mod.rs#L360-L363)
  * **[Golem](https://github.com/golemcloud/golem)** uses it to [render application manifest templates](https://github.com/golemcloud/golem/blob/78e34e9a97334bbb7a8fbb61303b0f9422824cfd/cli/golem-cli/src/model/template_render.rs) in its CLI

* Code Generation:
  * **[OpenTelemetry's Weaver](https://github.com/open-telemetry/weaver)** uses it to [generate documentation, code and other outputs](https://github.com/open-telemetry/weaver/blob/d49881445e09beb42e1a394bfa5f3068c660daf3/crates/weaver_forge/src/lib.rs#L482-L567) from the OTel specification.
  * **[Maturin](https://github.com/PyO3/maturin)** uses it to [generate project structures](https://github.com/PyO3/maturin/blob/e35097e6cf3b9115736e8ae208972178029a20d0/src/new_project.rs)
  * **[cargo-dist](https://github.com/axodotdev/cargo-dist)** uses it to [generate CI and project configuration](https://github.com/axodotdev/cargo-dist/blob/4cd61134863f54ca5a037400ebec71d039d42742/cargo-dist/src/backend/templates.rs)

* SQL Generation:
  * **[Spawn](https://docs.spawn.dev)** uses it [to render database migration scripts and tests](https://github.com/saward/spawn/blob/f9f3522343b61c383fca0ab88a83fd6788ca99a0/src/template.rs#L34-L100) and for [safe escaping of values](https://github.com/saward/spawn/blob/f9f3522343b61c383fca0ab88a83fd6788ca99a0/src/sql_formatter/postgres.rs).

## Similar Projects

These are related template engines for Rust:

* [Askama](https://crates.io/crates/askama): Jinja inspired, type-safe, requires template
  precompilation. Has significant divergence from Jinja syntax in parts.
* [Tera](https://crates.io/crates/tera): Jinja inspired, dynamic, has divergences from Jinja.
* [Liquid](https://crates.io/crates/liquid): an implementation of Liquid templates for Rust.
  Liquid was inspired by Django from which Jinja took its inspiration.
* [TinyTemplate](https://crates.io/crates/tinytemplate): minimal footprint template engine
  with syntax that takes lose inspiration from Jinja and handlebars.

## Sponsor

If you like the project and find it useful you can [become a
sponsor](https://github.com/sponsors/mitsuhiko).

## AI Use Disclaimer

This codebase mostly predates LLM based code generation but some recent features
have been built with support of AI.

## License and Links

- [Documentation](https://docs.rs/minijinja/)
- [Discussions](https://github.com/mitsuhiko/minijinja/discussions)
- [Examples](https://github.com/mitsuhiko/minijinja/tree/main/examples)
- [Issue Tracker](https://github.com/mitsuhiko/minijinja/issues)
- [MiniJinja Playground](https://mitsuhiko.github.io/minijinja/)
- [Updating Guide](UPDATING.md)
- License: [Apache-2.0](https://github.com/mitsuhiko/minijinja/blob/main/LICENSE)
