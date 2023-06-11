#![cfg(feature = "unstable_machinery")]
use minijinja::machinery::tokenize;

use std::fmt::Write;

#[test]
fn test_lexer() {
    insta::glob!("lexer-inputs/*.txt", |path| {
        let contents = std::fs::read_to_string(path).unwrap();

        let tokens: Result<Vec<_>, _> = tokenize(&contents, false, Default::default()).collect();
        insta::with_settings!({
            description => contents.trim_end(),
            omit_expression => true
        }, {
            let mut stringified = String::new();
            for (token, span) in tokens.unwrap() {
                let token_source = contents
                    .get(span.start_offset as usize..span.end_offset as usize)
                    .unwrap();
                writeln!(stringified, "{token:?}").unwrap();
                writeln!(stringified, "  {token_source:?}").unwrap();
            }
            insta::assert_snapshot!(&stringified);
        });
    });
}
