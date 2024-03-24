#![cfg(feature = "unstable_machinery")]
use minijinja::machinery::{Span, SyntaxConfig, Token, Tokenizer, WhitespaceConfig};
use minijinja::Error;

use std::fmt::Write;

pub fn tokenize(
    input: &str,
    syntax_config: SyntaxConfig,
    whitespace_config: WhitespaceConfig,
) -> Result<Vec<(Token<'_>, Span)>, Error> {
    let mut tokenizer = Tokenizer::new(input, false, syntax_config, whitespace_config);
    std::iter::from_fn(move || tokenizer.next_token().transpose()).collect()
}

fn stringify_tokens(tokens: Vec<(Token<'_>, Span)>, contents: &str) -> String {
    let mut stringified = String::new();
    for (token, span) in tokens {
        let token_source = contents
            .get(span.start_offset as usize..span.end_offset as usize)
            .unwrap();
        writeln!(stringified, "{token:?}").unwrap();
        writeln!(stringified, "  {token_source:?}").unwrap();
    }
    stringified
}

#[test]
fn test_lexer() {
    insta::glob!("lexer-inputs/*.txt", |path| {
        let contents = std::fs::read_to_string(path).unwrap();
        let tokens = tokenize(&contents, Default::default(), Default::default());
        insta::with_settings!({
            description => contents.trim_end(),
            omit_expression => true
        }, {
            let stringified = stringify_tokens(tokens.unwrap(), &contents);
            insta::assert_snapshot!(&stringified);
        });
    });
}

#[test]
fn test_trim_blocks() {
    let input = "{% block foo %}\nbar{% endblock %}";
    let tokens = tokenize(
        input,
        Default::default(),
        WhitespaceConfig {
            trim_blocks: true,
            ..Default::default()
        },
    );
    let stringified = stringify_tokens(tokens.unwrap(), input);
    insta::assert_snapshot!(&stringified, @r###"
    BlockStart
      "{%"
    Ident("block")
      "block"
    Ident("foo")
      "foo"
    BlockEnd
      "%}"
    TemplateData("bar")
      "bar"
    BlockStart
      "{%"
    Ident("endblock")
      "endblock"
    BlockEnd
      "%}"
    "###);
}
