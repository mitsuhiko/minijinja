use minijinja::{
    machinery::{parse, ToSource, WhitespaceConfig},
    syntax::SyntaxConfig,
};

#[test]
fn test_to_source() {
    let template = "{{ 'bar' if foobar else 'baz' }}";

    let ast = parse(
        template,
        "fn.tmpl",
        SyntaxConfig::default(),
        WhitespaceConfig::default(),
    )
    .unwrap();

    let mut source = String::new();
    ast.to_source(&mut source, 0).unwrap();

    insta::assert_snapshot!(source);
}
