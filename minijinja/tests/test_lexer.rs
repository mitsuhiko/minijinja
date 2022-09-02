#![cfg(feature = "unstable_machinery")]
use minijinja::machinery::tokenize;

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
            insta::assert_debug_snapshot!(&tokens);
        });
    });
}
