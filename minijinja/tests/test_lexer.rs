#![cfg(feature = "unstable_machinery")]
use minijinja::machinery::tokenize;

use std::fmt::Write;

#[test]
fn test_lexer() {
    insta::glob!("lexer-inputs/*.txt", |path| {
        let contents = std::fs::read_to_string(path).unwrap();
        let tokens: Result<Vec<_>, _> = tokenize(&contents, false).collect();
        let tokens = tokens.unwrap().into_iter().map(|x| x.0).collect::<Vec<_>>();
        insta::with_settings!({
            description => contents.trim_end(),
            omit_expression => true
        }, {
            let mut stringified = String::new();
            for token in tokens {
                writeln!(stringified, "{:?}", token).unwrap();
            }
            insta::assert_snapshot!(&stringified);
        });
    });
}
