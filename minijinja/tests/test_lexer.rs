#![cfg(feature = "unstable_machinery")]
use minijinja::machinery::{tokenize, Span};

use std::fmt::Write;

fn lookup_span(source: &str, span: Span) -> &str {
    let mut col = 0;
    let mut line = 1;
    let mut span_start = None;

    for (idx, c) in source.char_indices() {
        match span_start {
            Some(span_start) if span.end_line == line && span.end_col == col => {
                return &source[span_start..idx];
            }
            None if span.start_line == line && span.start_col == col => {
                span_start = Some(idx);
            }
            _ => {}
        }

        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    if let Some(span_start) = span_start {
        if span.end_line == line && span.end_col == col {
            return &source[span_start..];
        }
    }

    panic!("span {span:?} out of range")
}

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
                let token_source = lookup_span(&contents, span);
                writeln!(stringified, "{token:?}").unwrap();
                writeln!(stringified, "  {token_source:?}").unwrap();
            }
            insta::assert_snapshot!(&stringified);
        });
    });
}
