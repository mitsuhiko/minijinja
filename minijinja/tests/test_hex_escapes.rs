use minijinja::{Environment, render};

#[test]
fn test_hex_escape_sequences() {
    let env = Environment::new();
    
    // Test basic hex escape sequences
    assert_eq!(
        env.render_str(r#"{{ "\x41\x42\x43" }}"#, ()).unwrap(),
        "ABC"
    );
    
    // Test null byte
    assert_eq!(
        env.render_str(r#"{{ "\x00" }}"#, ()).unwrap(),
        "\0"
    );
    
    // Test space character
    assert_eq!(
        env.render_str(r#"{{ "\x20" }}"#, ()).unwrap(),
        " "
    );
    
    // Test lowercase hex
    assert_eq!(
        env.render_str(r#"{{ "\xff" }}"#, ()).unwrap(),
        "\u{ff}"
    );
    
    // Test uppercase hex
    assert_eq!(
        env.render_str(r#"{{ "\xFF" }}"#, ()).unwrap(),
        "\u{FF}"
    );
    
    // Test mixed with other escape sequences
    assert_eq!(
        env.render_str(r#"{{ "Hello\x20\x57\x6f\x72\x6c\x64\x21" }}"#, ()).unwrap(),
        "Hello World!"
    );
    
    // Test with regular escapes
    assert_eq!(
        env.render_str(r#"{{ "Line1\nLine2\x20\x2d\x20Tab:\t\x41" }}"#, ()).unwrap(),
        "Line1\nLine2 - Tab:\tA"
    );
}

#[test]
fn test_hex_escape_errors() {
    let env = Environment::new();
    
    // Test invalid hex sequences should error
    assert!(env.render_str(r#"{{ "\x" }}"#, ()).is_err());
    assert!(env.render_str(r#"{{ "\x1" }}"#, ()).is_err());
    assert!(env.render_str(r#"{{ "\xGG" }}"#, ()).is_err());
    assert!(env.render_str(r#"{{ "\xZ1" }}"#, ()).is_err());
}

#[test]
fn test_hex_escape_in_expressions() {
    let env = Environment::new();
    
    // Test in string concatenation
    assert_eq!(
        env.render_str(r#"{{ "A" ~ "\x42" ~ "C" }}"#, ()).unwrap(),
        "ABC"
    );
    
    // Test in conditionals
    assert_eq!(
        env.render_str(r#"{% if "\x41" == "A" %}Match{% endif %}"#, ()).unwrap(),
        "Match"
    );
}

#[test]
fn test_render_macro() {
    // Test with the render! macro
    assert_eq!(
        render!(r#"{{ "\x48\x65\x6c\x6c\x6f" }}"#),
        "Hello"
    );
    
    assert_eq!(
        render!(r#"{{ "\x57\x6f\x72\x6c\x64\x21" }}"#),
        "World!"
    );
}