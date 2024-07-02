use std::io::Write;
use std::process::Command;

use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};
use tempfile::NamedTempFile;

fn cli() -> Command {
    Command::new(get_cargo_bin("minijinja-cli"))
}

fn file_with_contents(contents: &str) -> NamedTempFile {
    file_with_contents_and_ext(contents, "")
}

fn file_with_contents_and_ext(contents: &str, ext: &str) -> NamedTempFile {
    let mut f = tempfile::Builder::new()
        .prefix("minijinja-testfile--")
        .suffix(ext)
        .tempfile()
        .unwrap();
    f.write_all(contents.as_bytes()).unwrap();
    f
}

macro_rules! bind_common_filters {
    ($($expr:expr),*) => {
        let mut settings = insta::Settings::clone_current();
        settings.add_filter(r"(?m)(non-existing template).*$", "$1 [FILENAME HERE]");
        settings.add_filter(r"(?mis)(^Referenced variables: \{).*\z", "$1 ... }");
        settings.add_filter(
            r"(?m)^-+ (minijinja-testfile--\S+) -+$",
            "--- [TEMPLATE] ---",
        );
        settings.add_filter(
            r"\(in .*minijinja-testfile--.*?:(\d+)\)",
            "(in [TEMPLATE]:$1)",
        );
        let _guard = settings.bind_to_scope();
    };
}

#[test]
fn test_explicit_format() {
    let input = file_with_contents(r#"{"foo": "bar"}"#);
    let tmpl = file_with_contents(r#"Hello {{ foo }}!"#);

    assert_cmd_snapshot!(
        cli()
            .arg("--format=json")
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!

    ----- stderr -----
    "###);
}

#[test]
fn test_no_newline() {
    let input = file_with_contents(r#"{"foo": "bar"}"#);
    let tmpl = file_with_contents(r#"Hello {{ foo }}!"#);

    assert_cmd_snapshot!(
        cli()
            .arg("--format=json")
            .arg("--no-newline")
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!
    ----- stderr -----
    "###);
}

#[test]
fn test_json() {
    let input = file_with_contents_and_ext(r#"{"foo": "bar"}"#, ".json");
    let tmpl = file_with_contents(r#"Hello {{ foo }}!"#);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!

    ----- stderr -----
    "###);
}

#[test]
#[cfg(feature = "json5")]
fn test_json5() {
    let input = file_with_contents_and_ext(r#"/* foo */{"foo": "bar"}"#, ".json");
    let tmpl = file_with_contents(r#"Hello {{ foo }}!"#);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!

    ----- stderr -----
    "###);
}

#[test]
#[cfg(feature = "yaml")]
fn test_yaml() {
    let input = file_with_contents_and_ext(r#"foo: bar"#, ".yaml");
    let tmpl = file_with_contents(r#"Hello {{ foo }}!"#);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!

    ----- stderr -----
    "###);
}

#[test]
#[cfg(feature = "yaml")]
fn test_yaml_aliases() {
    let input = file_with_contents_and_ext(
        r#"
a: &a
  key1: value1

b: &b
  key2: value2

c:
  <<: *a
  key2: from-c

d:
  <<: [*a, *b]
  key3: value3
"#,
        ".yaml",
    );
    let tmpl = file_with_contents(r#"{{ [c.key1, c.key2] }}\n{{ [d.key1, d.key2, d.key3] }}"#);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    ["value1", "from-c"]\n["value1", "value2", "value3"]

    ----- stderr -----
    "###);
}

#[test]
#[cfg(feature = "toml")]
fn test_toml() {
    let input = file_with_contents_and_ext("[section]\nfoo = \"bar\"", ".toml");
    let tmpl = file_with_contents(r#"Hello {{ section.foo }}!"#);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!

    ----- stderr -----
    "###);

    let mut tmpl = NamedTempFile::new().unwrap();
    tmpl.write_all(br#"Hello {{ foo }}!"#).unwrap();

    assert_cmd_snapshot!(
        cli()
            .arg("--select=section")
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!

    ----- stderr -----
    "###);
}

#[test]
#[cfg(feature = "querystring")]
fn test_querystring() {
    let input = file_with_contents_and_ext("foo=blub+blah%2fx", ".qs");
    let tmpl = file_with_contents(r#"Hello {{ foo }}!"#);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello blub blah/x!

    ----- stderr -----
    "###);
}

#[test]
#[cfg(feature = "ini")]
fn test_ini() {
    let input = file_with_contents_and_ext("[section]\nfoo = bar", ".ini");
    let tmpl = file_with_contents(r#"Hello {{ section.foo }}!"#);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!

    ----- stderr -----
    "###);

    let input = file_with_contents_and_ext("foo = bar", ".ini");
    let mut tmpl = NamedTempFile::new().unwrap();
    tmpl.write_all(br#"Hello {{ foo }}!"#).unwrap();

    assert_cmd_snapshot!(
        cli()
            .arg("--select=default")
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!

    ----- stderr -----
    "###);
}

#[test]
fn test_context_stdin() {
    let tmpl = file_with_contents(r#"Hello {{ foo }}!"#);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg("-")
            .arg("--format=json")
            .pass_stdin(r#"{"foo": "bar"}"#),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!

    ----- stderr -----
    "###);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg("-")
            .pass_stdin(r#""#),
        @r###"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    error: auto detection does not work with data from stdin
    "###);
}

#[test]
fn test_dump() {
    let tmpl = file_with_contents(r#"Hello {{ foo }}!"#);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg("--dump=tokens"),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    TemplateData("Hello ")
    VariableStart
    Ident("foo")
    VariableEnd
    TemplateData("!")

    ----- stderr -----
    "###);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg("--dump=ast"),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Template {
        children: [
            EmitRaw {
                raw: "Hello ",
            } @ 1:0-1:6,
            EmitExpr {
                expr: Var {
                    id: "foo",
                } @ 1:9-1:12,
            } @ 1:6-1:12,
            EmitRaw {
                raw: "!",
            } @ 1:15-1:16,
        ],
    } @ 0:0-1:16

    ----- stderr -----
    "###);

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg("--dump=instructions"),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Block: "<root>"
         0: EmitRaw("Hello ")
         1: Lookup("foo")
         2: Emit
         3: EmitRaw("!")

    ----- stderr -----
    "###);
}

#[test]
fn test_include() {
    let tmpl = file_with_contents(r#"{% include ENV.OTHER_TEMPLATE %}"#);
    let other_tmpl = file_with_contents(r#"Hello!"#);
    let input = file_with_contents_and_ext(r#"{}"#, ".json");

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg(input.path())
            .arg("--env")
            .env("OTHER_TEMPLATE", other_tmpl.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello!

    ----- stderr -----
    "###);
}

#[test]
fn test_no_include() {
    let tmpl = file_with_contents(r#"{% include ENV.OTHER_TEMPLATE %}"#);
    let other_tmpl = file_with_contents(r#"Hello!"#);
    let input = file_with_contents_and_ext(r#"{}"#, ".json");

    bind_common_filters!();

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg(input.path())
            .arg("--env")
            .env("OTHER_TEMPLATE", other_tmpl.path())
            .arg("--no-include"),
        @r###"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    error: template not found: tried to include non-existing template [FILENAME HERE]

    --- [TEMPLATE] ---
       1 > {% include ENV.OTHER_TEMPLATE %}
         i    ^^^^^^^^^^^^^^^^^^^^^^^^^^ template not found
    ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    Referenced variables: { ... }
    "###);
}

#[test]
fn test_syntax_error() {
    let tmpl = file_with_contents("{{ all_good }}\n{% for item in seq");
    let input = file_with_contents_and_ext(r#"{}"#, ".json");

    bind_common_filters!();

    assert_cmd_snapshot!(
        cli()
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    error: syntax error: unexpected end of input, expected end of block (in [TEMPLATE]:2)

    --- [TEMPLATE] ---
       1 | {{ all_good }}
       2 > {% for item in seq
         i                ^^^ syntax error
    ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    No referenced variables
    -------------------------------------------------------------------------------
    "###);
}

#[test]
fn test_stdin_template() {
    let input = file_with_contents_and_ext(r#"{"foo": "bar"}"#, ".json");

    assert_cmd_snapshot!(
        cli()
            .arg("-")
            .arg(input.path())
            .pass_stdin("Hello {{ foo }}!"),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Hello bar!

    ----- stderr -----
    "###);
}

#[test]
fn test_line_statement() {
    let tmpl = file_with_contents("# for item in seq\n  {{ item }}\n# endfor");
    let input = file_with_contents_and_ext(r#"{"seq": [1, 2, 3]}"#, ".json");

    assert_cmd_snapshot!(
        cli()
            .arg("-sline-statement-prefix=#")
            .arg("--no-newline")
            .arg(tmpl.path())
            .arg(input.path()),
        @r###"
    success: true
    exit_code: 0
    ----- stdout -----
      1
      2
      3

    ----- stderr -----
    "###);
}
